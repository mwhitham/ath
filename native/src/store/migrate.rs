//! Ordered SQL migrations.
//!
//! The published SQL files in `spec/sql/migrations/` are the executable
//! migrations. They are embedded unchanged. New databases run the same ordered
//! chain as upgrades, so there is no separate fresh-schema definition.

use rusqlite::Transaction;
use serde::Serialize;

use super::encode::{self, Layout};
use crate::error::{Error, Result};

/// The SQL schema step this software writes.
pub const SQL_SCHEMA_VERSION: u32 = 5;

/// One published migration step.
pub struct MigrationStep {
    pub version: u32,
    pub file: &'static str,
    pub description: &'static str,
    pub sql: &'static str,
}

/// The ordered migration chain.
pub const MIGRATION_CHAIN: &[MigrationStep] = &[
    MigrationStep {
        version: 1,
        file: "migrations/001-initial.sql",
        description: "Initial Athletic Standard v1 schema, enforcement triggers, and stable read views.",
        sql: include_str!("../../../spec/sql/migrations/001-initial.sql"),
    },
    MigrationStep {
        version: 2,
        file: "migrations/002-provider-encoding-repair.sql",
        description: "Repair typed-table JSON bytes after npm 1.0.0 provider imports without changing record content.",
        sql: include_str!("../../../spec/sql/migrations/002-provider-encoding-repair.sql"),
    },
    MigrationStep {
        version: 3,
        file: "migrations/003-version-2.sql",
        description: "Upgrade the database to software version 2.0.0. Adds workout names, text, and route points. History, links, attachments, and deletion records stay.",
        sql: include_str!("../../../spec/sql/migrations/003-version-2.sql"),
    },
    MigrationStep {
        version: 4,
        file: "migrations/004-version-2-1.sql",
        description: "Upgrade the database to software version 2.1.0. No column is added. Records written before this step stay as they are.",
        sql: include_str!("../../../spec/sql/migrations/004-version-2-1.sql"),
    },
    MigrationStep {
        version: 5,
        file: "migrations/005-version-2-1-1.sql",
        description: "Upgrade the database to software version 2.1.1. No athlete-owned column changes. Adds internal recovery state for attachment file changes.",
        sql: include_str!("../../../spec/sql/migrations/005-version-2-1-1.sql"),
    },
];

/// What a migration would do, shown before any change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationPreview {
    pub migration_needed: bool,
    pub source_sql_schema_version: u32,
    pub target_sql_schema_version: u32,
    pub database_id: String,
    pub planned_steps: Vec<MigrationStepPreview>,
    pub planned_backup_path: Option<String>,
    pub compatibility_effect: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationStepPreview {
    pub target_sql_schema_version: u32,
    pub migration_file: String,
    pub description: String,
}

pub fn planned_steps(source_version: u32) -> Vec<MigrationStepPreview> {
    MIGRATION_CHAIN
        .iter()
        .filter(|step| step.version > source_version && step.version <= SQL_SCHEMA_VERSION)
        .map(|step| MigrationStepPreview {
            target_sql_schema_version: step.version,
            migration_file: step.file.to_string(),
            description: step.description.to_string(),
        })
        .collect()
}

/// The preview for a database at `source_version`.
pub fn preview(source_version: u32, database_id: &str, planned_backup_path: Option<String>) -> MigrationPreview {
    let steps = planned_steps(source_version);
    let needed = source_version < SQL_SCHEMA_VERSION;
    let compatibility_effect = if !needed {
        "No migration is required. Direct-read views and writable tables match this ath version.".to_string()
    } else if steps.is_empty() {
        format!(
            "No ordered migration exists from SQL schema {source_version} to {SQL_SCHEMA_VERSION}. Update ath to a version that supports this database, or restore from a portable JSON package."
        )
    } else {
        "The database will be upgraded in place after a verified SQLite backup. Older direct-SQL clients may need the sql_vN_* views documented for the new schema version.".to_string()
    };
    MigrationPreview {
        migration_needed: needed,
        source_sql_schema_version: source_version,
        target_sql_schema_version: SQL_SCHEMA_VERSION,
        database_id: database_id.to_string(),
        planned_backup_path: if needed && !steps.is_empty() { planned_backup_path } else { None },
        planned_steps: steps,
        compatibility_effect,
    }
}

/// Run one migration step inside the caller's write transaction.
///
/// Step 2 first rebuilds typed rows whose stored JSON differs from the current
/// revision only in key order. Any other difference stops the migration, so a
/// person can review it with `ath repair`.
pub fn apply_step(tx: &Transaction<'_>, version: u32) -> Result<StepOutcome> {
    if version == 1 {
        return Err(Error::other("The initial SQL schema cannot be applied to an existing Athletic Standard database"));
    }
    let step = MIGRATION_CHAIN
        .iter()
        .find(|step| step.version == version)
        .ok_or_else(|| Error::other(format!("No migration is registered for SQL schema version {version}")))?;
    let mut repaired = 0;
    if version == 2 {
        repaired = repair_provider_encoding_rows(tx)?;
    }
    tx.execute_batch(step.sql)?;
    tx.pragma_update(None, "user_version", version)?;
    Ok(StepOutcome { repaired_records: repaired })
}

pub struct StepOutcome {
    /// Records whose typed rows step 2 rebuilt. A repair bumps the database revision.
    pub repaired_records: usize,
}

/// Rebuild typed rows whose JSON text differs from the current revision only
/// in key order (migration step 2).
pub fn repair_provider_encoding_rows(tx: &Transaction<'_>) -> Result<usize> {
    let layout = Layout { sql_schema_version: 1 };
    let mut ids = Vec::new();
    for record_type in crate::model::RecordType::ALL {
        let table = encode::table_for(*record_type);
        let mut statement = tx.prepare(&format!(
            "SELECT records.record_id, record_revisions.record_json, {table}.data_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             JOIN {table} ON {table}.record_id = records.record_id
             WHERE records.record_type = ?1 AND {table}.data_json <> record_revisions.record_json"
        ))?;
        let rows = statement.query_map([record_type.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        for row in rows {
            let (id, record_json, typed_json) = row?;
            let same = match (crate::json::parse(&record_json), crate::json::parse(&typed_json)) {
                (Ok(left), Ok(right)) => crate::json::semantic_eq(&left, &right),
                _ => false,
            };
            if !same {
                return Err(Error::other(format!(
                    "record {id} has a typed-table mismatch that is not key-order only; run ath repair --from-revision after reviewing affected records, or restore from backup"
                )));
            }
            ids.push(id);
        }
    }
    for id in &ids {
        encode::rebuild_from_current_revision(tx, layout, id)?;
    }
    Ok(ids.len())
}
