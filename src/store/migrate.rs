//! Ordered SQL migrations.
//!
//! The published SQL files in `spec/sql/migrations/` are the executable
//! migrations. They are embedded unchanged. New databases run the same ordered
//! chain as upgrades, so there is no separate fresh-schema definition.

use std::path::{Path, PathBuf};

use rusqlite::Transaction;
use serde::Serialize;

use super::backup::{self, VerifiedBackup};
use super::encode::{self, Layout};
use super::{read_metadata, DatabaseMetadata, OpenOptions, Store};
use crate::error::{Error, Result};
use crate::files;
use crate::model::time;

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
        sql: include_str!("../../spec/sql/migrations/001-initial.sql"),
    },
    MigrationStep {
        version: 2,
        file: "migrations/002-provider-encoding-repair.sql",
        description: "Repair typed-table JSON bytes after npm 1.0.0 provider imports without changing record content.",
        sql: include_str!("../../spec/sql/migrations/002-provider-encoding-repair.sql"),
    },
    MigrationStep {
        version: 3,
        file: "migrations/003-version-2.sql",
        description: "Upgrade the database to software version 2.0.0. Adds workout names, text, and route points. History, links, attachments, and deletion records stay.",
        sql: include_str!("../../spec/sql/migrations/003-version-2.sql"),
    },
    MigrationStep {
        version: 4,
        file: "migrations/004-version-2-1.sql",
        description: "Upgrade the database to software version 2.1.0. No column is added. Records written before this step stay as they are.",
        sql: include_str!("../../spec/sql/migrations/004-version-2-1.sql"),
    },
    MigrationStep {
        version: 5,
        file: "migrations/005-version-2-1-1.sql",
        description: "Upgrade the database to software version 2.1.1. No athlete-owned column changes. Adds internal recovery state for attachment file changes.",
        sql: include_str!("../../spec/sql/migrations/005-version-2-1-1.sql"),
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

// ---- The confirmed migration --------------------------------------------

/// The `ath migrate` preview: the file, the plan, and `write_confirmed: false`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationCommandPreview {
    pub file: String,
    #[serde(flatten)]
    pub preview: MigrationPreview,
    pub write_confirmed: bool,
}

/// Options for a confirmed migration.
#[derive(Debug, Clone, Default)]
pub struct MigrationOptions {
    /// The reviewed backup path from the preview. Without it, a new default
    /// name is planned.
    pub backup_path: Option<PathBuf>,
    /// The time used to name a default backup.
    pub now_millis: Option<i64>,
}

/// The result of a confirmed migration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfirmedMigrationResult {
    /// The verified backup, or null when no migration was needed.
    pub backup_path: Option<String>,
    pub migrated: bool,
    pub preview: MigrationPreview,
}

/// The preview for an open database. Reading it changes nothing. A backup
/// path is planned only when a migration can run.
pub fn migration_preview(store: &Store, now_millis: Option<i64>) -> Result<MigrationPreview> {
    let metadata = read_metadata(store.connection())?;
    Ok(preview_for(&metadata, store.path(), now_millis))
}

fn preview_for(metadata: &DatabaseMetadata, path: &Path, now_millis: Option<i64>) -> MigrationPreview {
    let needed = metadata.sql_schema_version < SQL_SCHEMA_VERSION;
    let backup = if needed && !planned_steps(metadata.sql_schema_version).is_empty() {
        let now = now_millis.unwrap_or_else(time::now_millis);
        Some(backup::default_migration_backup_path(path, now).display().to_string())
    } else {
        None
    };
    preview(metadata.sql_schema_version, &metadata.database_id, backup)
}

/// Inspect a database file for `ath migrate` without changing it. The file
/// is opened read-only, so its journal mode, header, and data stay as they are.
pub fn inspect_migration(path: &Path, now_millis: Option<i64>) -> Result<MigrationCommandPreview> {
    let store = Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: true })?;
    let preview = migration_preview(&store, now_millis)?;
    let file = store.path().display().to_string();
    drop(store);
    Ok(MigrationCommandPreview { file, preview, write_confirmed: false })
}

/// Make a verified backup, then apply every pending step in one write
/// transaction.
///
/// The store must be write-capable and opened with `allow_outdated_schema`.
/// Under the write lock, the database revision, database ID, schema step, and
/// file identity must still match the backup. A change in between makes the
/// backup stale, so nothing is migrated and the caller must preview again.
/// Any other failure rolls the complete migration back and names the
/// verified backup. On success the store's metadata is current.
pub fn execute_confirmed_migration(store: &mut Store, options: &MigrationOptions) -> Result<ConfirmedMigrationResult> {
    if store.is_read_only() {
        return Err(Error::refused("This database was opened read-only"));
    }
    let path = store.path().to_path_buf();
    let metadata = read_metadata(store.connection())?;
    let preview = preview_for(&metadata, &path, options.now_millis);
    if !preview.migration_needed {
        return Ok(ConfirmedMigrationResult { backup_path: None, migrated: false, preview });
    }
    if preview.planned_steps.is_empty() {
        return Err(Error::refused(preview.compatibility_effect.clone()));
    }
    let backup_path = match (&options.backup_path, &preview.planned_backup_path) {
        (Some(path), _) => path.clone(),
        (None, Some(planned)) => PathBuf::from(planned),
        (None, None) => return Err(Error::other("Migration backup path is missing")),
    };

    let identity = files::identity(&path)?;
    let verified = backup::create_verified_backup(store.connection(), &backup_path, &metadata)?;
    files::assert_same_file(&path, identity)?;
    apply_after_backup(store, preview, &verified, identity)
}

/// Migrate with a verified backup made earlier, for example by
/// `Store::create_verified_backup`. The backup must still describe the
/// database: under the write lock, its database ID, schema step, and database
/// revision must match. Otherwise nothing is migrated.
pub fn migrate_with_verified_backup(store: &mut Store, verified: &VerifiedBackup) -> Result<ConfirmedMigrationResult> {
    if store.is_read_only() {
        return Err(Error::refused("This database was opened read-only"));
    }
    let path = store.path().to_path_buf();
    let identity = files::identity(&path)?;
    let metadata = read_metadata(store.connection())?;
    let mut preview = preview(metadata.sql_schema_version, &metadata.database_id, None);
    if !preview.migration_needed {
        return Ok(ConfirmedMigrationResult { backup_path: None, migrated: false, preview });
    }
    if preview.planned_steps.is_empty() {
        return Err(Error::refused(preview.compatibility_effect.clone()));
    }
    preview.planned_backup_path = Some(verified.path.display().to_string());
    apply_after_backup(store, preview, verified, identity)
}

/// Apply the given steps in order inside the caller's write transaction and
/// check that each one reached its schema step.
fn apply_steps(tx: &Transaction<'_>, steps: &[u32]) -> Result<()> {
    let mut repaired = 0;
    for version in steps {
        repaired += apply_step(tx, *version)?.repaired_records;
        let after = read_metadata(tx)?;
        if after.sql_schema_version != *version {
            return Err(Error::other(format!("Migration to SQL schema {version} did not update database metadata")));
        }
        let user_version: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version != *version {
            return Err(Error::other(format!(
                "SQLite user_version {user_version} does not match SQL schema {version} after migration"
            )));
        }
    }
    if repaired > 0 {
        // Step 2 rebuilt typed rows, so the database changed.
        tx.execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])?;
    }
    let last = read_metadata(tx)?;
    if last.sql_schema_version != SQL_SCHEMA_VERSION {
        return Err(Error::other(format!(
            "Migration finished at SQL schema {}, expected {SQL_SCHEMA_VERSION}",
            last.sql_schema_version
        )));
    }
    Ok(())
}

/// Apply every pending step in one write transaction without making a
/// backup. This is the 2.1.0 `applyPendingDatabaseMigrations` export for
/// callers that made their own backup; `ath migrate` always uses
/// [`execute_confirmed_migration`].
pub fn apply_pending_without_backup(store: &mut Store) -> Result<()> {
    let metadata = read_metadata(store.connection())?;
    if metadata.sql_schema_version >= SQL_SCHEMA_VERSION {
        return Ok(());
    }
    let steps: Vec<u32> =
        planned_steps(metadata.sql_schema_version).iter().map(|step| step.target_sql_schema_version).collect();
    if steps.is_empty() {
        return Err(Error::other(format!(
            "No migration is available from SQL schema {} to {SQL_SCHEMA_VERSION}",
            metadata.sql_schema_version
        )));
    }
    store.transaction(|tx| apply_steps(tx, &steps))?;
    store.reload_metadata()
}

fn apply_after_backup(
    store: &mut Store,
    preview: MigrationPreview,
    verified: &VerifiedBackup,
    identity: files::FileIdentity,
) -> Result<ConfirmedMigrationResult> {
    crate::package::failpoint!("migrate.after_backup");
    let path = store.path().to_path_buf();
    let backup_text = verified.path.display().to_string();
    let stale = format!(
        "Database revision changed before migration could start. Nothing was migrated. The backup at {backup_text} is older than the database, so preview the migration again to make a new backup."
    );
    let steps: Vec<u32> = preview.planned_steps.iter().map(|step| step.target_sql_schema_version).collect();

    let outcome = store.transaction(|tx| {
        files::assert_same_file(&path, identity).map_err(|_| Error::stale(stale.clone()))?;
        let current = read_metadata(tx)?;
        if super::database_revision(tx)? != verified.database_revision
            || current.database_id != verified.database_id
            || current.sql_schema_version != verified.sql_schema_version
        {
            return Err(Error::stale(stale.clone()));
        }
        apply_steps(tx, &steps)?;
        crate::package::failpoint!("migrate.before_commit");
        Ok(())
    });

    match outcome {
        Ok(()) => {
            crate::package::failpoint!("migrate.after_commit");
            store.reload_metadata()?;
            Ok(ConfirmedMigrationResult { backup_path: Some(backup_text), migrated: true, preview })
        }
        Err(Error::Stale(message)) if message == stale => Err(Error::Stale(message)),
        Err(error) => {
            let changed = super::database_revision(store.connection())
                .map(|revision| revision != verified.database_revision)
                .unwrap_or(false);
            let detail = if changed {
                format!("Cause: {error}. The database revision changed; restore from the verified backup.")
            } else {
                format!("Cause: {error}. The database revision did not change.")
            };
            Err(Error::other(format!("Migration failed after creating verified backup at {backup_text}. {detail}")))
        }
    }
}
