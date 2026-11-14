//! Opening, creating, and inspecting a database file.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::json::{self, Value};
use crate::model::time;
use crate::protocol::definition::{Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};
use crate::store::{CreateOptions, DatabaseMetadata, OpenOptions, Store};

use super::input;

const OPENED: &str = "{ path: string; database_id: string; sql_schema_version: number; warnings: string[] }";

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "database.open",
        kind: Kind::Async,
        target: TargetUse::None,
        input:
            "{ path: string; allow_outdated_schema?: boolean; defer_wal?: boolean; now_iso: string; sequence: number }",
        output: OPENED,
        items: None,
        callbacks: &[],
        run: open,
    },
    Operation {
        name: "database.inspect",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ path: string }",
        output: OPENED,
        items: None,
        callbacks: &[],
        run: inspect,
    },
    Operation {
        name: "database.create",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ path: string; database_id?: unknown; created_at?: unknown }",
        output: OPENED,
        items: None,
        callbacks: &[],
        run: create,
    },
    Operation {
        name: "database.metadata",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "Record<string, never>",
        output: "Api.DatabaseMetadata",
        items: None,
        callbacks: &[],
        run: metadata,
    },
    Operation {
        name: "database.apply_migrations",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "Record<string, never>",
        output: "null",
        items: None,
        callbacks: &[],
        run: apply_migrations,
    },
    Operation {
        name: "database.migrate_confirmed",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ backup_path?: string; now_iso: string; sequence: number }",
        output: "Api.ConfirmedMigrationResult",
        items: None,
        callbacks: &[],
        run: migrate_confirmed,
    },
    Operation {
        name: "database.backup",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ source_path: string; backup_path: string; source_metadata: unknown }",
        output: "null",
        items: None,
        callbacks: &[],
        run: backup,
    },
    Operation {
        name: "database.check",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "Record<string, never>",
        output: "Api.DatabaseCheckResult",
        items: None,
        callbacks: &[],
        run: check,
    },
    Operation {
        name: "database.stats",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "Record<string, never>",
        output: "Api.DatabaseStats",
        items: None,
        callbacks: &[],
        run: stats,
    },
    Operation {
        name: "repair.candidates",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "Record<string, never>",
        output: "{ encoding_record_count: number; revision_align_record_ids: string[] }",
        items: None,
        callbacks: &[],
        run: repair_candidates,
    },
    Operation {
        name: "repair.provider_encoding",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "Record<string, never>",
        output: "Api.ProviderEncodingRepairResult",
        items: None,
        callbacks: &[],
        run: repair_provider_encoding,
    },
    Operation {
        name: "repair.confirmed",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ backup_path?: string; now_ms?: number; from_revision?: boolean }",
        output: "Api.ConfirmedProviderRepairResult",
        items: None,
        callbacks: &[],
        run: repair_confirmed,
    },
    Operation {
        name: "database.release",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "Record<string, never>",
        output: "null",
        items: None,
        callbacks: &[],
        run: release,
    },
];

fn opened(store: &Store, warnings: Vec<String>) -> Value {
    serde_json::json!({
        "path": store.path().display().to_string(),
        "database_id": store.metadata().database_id,
        "sql_schema_version": store.metadata().sql_schema_version,
        "warnings": warnings,
    })
}

/// `openDatabase(target, options)`: a read-write open that refuses a database
/// needing migration unless allowed, then switches to WAL unless deferred.
fn open(call: &mut Call<'_>) -> Outcome {
    let path = input::required_string(&call.input, "path")?;
    let allow_outdated_schema = input::flag(&call.input, "allow_outdated_schema")?;
    let defer_wal = input::flag(&call.input, "defer_wal")?;
    let store = match Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema }) {
        Ok(store) => store,
        Err(crate::Error::MigrationRequired(preview)) => {
            // The planned backup name uses the client's counter, as 2.1.0 did
            // within one process.
            let mut preview = *preview;
            if preview.planned_backup_path.is_some() {
                let iso = input::required_string(&call.input, "now_iso")?;
                let sequence = input::get(&call.input, "sequence").and_then(input::whole_number).unwrap_or(1);
                let absolute = crate::files::absolute(std::path::Path::new(&path))?;
                preview.planned_backup_path = Some(
                    crate::store::backup::migration_backup_path(&absolute, &iso, sequence as u32).display().to_string(),
                );
            }
            return Err(crate::Error::MigrationRequired(Box::new(preview)).into());
        }
        Err(error) => return Err(error.into()),
    };
    if !defer_wal {
        store.enable_wal()?;
    }
    let warnings = store.warnings.clone();
    let out = opened(&store, warnings);
    store.close()?;
    Ok(out)
}

/// `inspectDatabase(target)`: a read-only open for migration previews. It
/// never changes the journal mode and accepts an older schema step.
fn inspect(call: &mut Call<'_>) -> Outcome {
    let path = input::required_string(&call.input, "path")?;
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true })?;
    let warnings = store.warnings.clone();
    let out = opened(&store, warnings);
    store.close()?;
    Ok(out)
}

/// `createDatabase(target, options)`.
fn create(call: &mut Call<'_>) -> Outcome {
    let path = input::required_string(&call.input, "path")?;
    let database_id =
        input::checked_text_or("DatabaseId", input::get(&call.input, "database_id"), crate::ids::new_record_id)?;
    let created_at = input::checked_text_or("UtcTimestamp", input::get(&call.input, "created_at"), time::now_utc)?;
    let store = Store::create(&path, CreateOptions { database_id: Some(database_id), created_at: Some(created_at) })?;
    let out = opened(&store, Vec::new());
    store.close()?;
    Ok(out)
}

/// `readDatabaseMetadata(database)`.
fn metadata(call: &mut Call<'_>) -> Outcome {
    let store = call.store()?;
    Ok(json::to_value(store.metadata()))
}

/// `releaseLocalDatabase(database)`: closing a read-write connection
/// checkpoints the WAL and truncates it.
fn release(call: &mut Call<'_>) -> Outcome {
    call.store()?;
    Ok(Value::Null)
}

/// `applyPendingDatabaseMigrations(database)` and `migrateDatabase(database)`:
/// every pending step in one transaction, without a backup.
fn apply_migrations(call: &mut Call<'_>) -> Outcome {
    crate::store::migrate::apply_pending_without_backup(call.store()?)?;
    Ok(Value::Null)
}

/// `executeConfirmedDatabaseMigration(database, options)`: a verified backup,
/// then every pending step in one transaction.
fn migrate_confirmed(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let backup_path = match input::string(&request, "backup_path")? {
        Some(path) => Some(PathBuf::from(path)),
        None => {
            // The default name uses the client's time and counter.
            let store = call.store()?;
            let metadata = store.metadata().clone();
            let needed = metadata.sql_schema_version < crate::store::SQL_SCHEMA_VERSION;
            let steps = crate::store::migrate::planned_steps(metadata.sql_schema_version);
            if needed && !steps.is_empty() {
                let iso = input::required_string(&request, "now_iso")?;
                let sequence = input::get(&request, "sequence").and_then(input::whole_number).unwrap_or(1);
                Some(crate::store::backup::migration_backup_path(store.path(), &iso, sequence as u32))
            } else {
                None
            }
        }
    };
    let options = crate::store::migrate::MigrationOptions { backup_path, now_millis: None };
    let result = crate::store::migrate::execute_confirmed_migration(call.store()?, &options)?;
    Ok(json::to_value(&result))
}

fn metadata_from(value: &Value) -> Result<DatabaseMetadata, Failure> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    let number = value.get("sql_schema_version").and_then(input::whole_number);
    match (
        text("application_marker"),
        number,
        text("athleticstandard_version"),
        text("database_id"),
        text("created_at"),
    ) {
        (
            Some(application_marker),
            Some(sql_schema_version),
            Some(athleticstandard_version),
            Some(database_id),
            Some(created_at),
        ) => Ok(DatabaseMetadata {
            application_marker,
            sql_schema_version: sql_schema_version as u32,
            athleticstandard_version,
            database_id,
            created_at,
        }),
        _ => Err(Failure::Input("source_metadata must be database metadata".into())),
    }
}

/// `createVerifiedSqliteBackup(source, sourcePath, backupPath, sourceMetadata)`:
/// copy the database at `sourcePath` with the SQLite backup API and verify
/// the copy against `sourceMetadata`. The source opens read-only.
fn backup(call: &mut Call<'_>) -> Outcome {
    let source_path = input::required_string(&call.input, "source_path")?;
    let backup_path = input::required_string(&call.input, "backup_path")?;
    let expected = metadata_from(input::required(&call.input, "source_metadata")?)?;
    let store = Store::open(&source_path, OpenOptions { read_only: true, allow_outdated_schema: true })?;
    let result = crate::store::backup::create_verified_backup(store.connection(), Path::new(&backup_path), &expected);
    store.close()?;
    result?;
    Ok(Value::Null)
}

/// `checkDatabase(database)`: one read snapshot; nothing is written.
fn check(call: &mut Call<'_>) -> Outcome {
    let result = crate::store::check::check_database(call.store()?)?;
    Ok(json::to_value(&result))
}

/// `databaseStats(database)`.
fn stats(call: &mut Call<'_>) -> Outcome {
    let stats = crate::store::stats::database_stats(call.store()?)?;
    Ok(json::to_value(&stats))
}

/// The repair counts and the records a revision-align repair would rebuild.
fn repair_candidates(call: &mut Call<'_>) -> Outcome {
    let store = call.store()?;
    let candidates = crate::store::repair::repair_candidates(store.connection(), store.layout())?;
    Ok(serde_json::json!({
        "encoding_record_count": candidates.repair_candidates(),
        "revision_align_record_ids": candidates.revision_align_record_ids,
    }))
}

/// `repairProviderEncoding(database)`: rebuild typed rows whose JSON differs
/// from the current revision only in encoding.
fn repair_provider_encoding(call: &mut Call<'_>) -> Outcome {
    let result = crate::store::repair::repair_provider_encoding(call.store()?)?;
    Ok(json::to_value(&result))
}

/// `executeConfirmedProviderEncodingRepair(database, options)`.
fn repair_confirmed(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let now_millis = match input::get(&request, "now_ms") {
        None => None,
        Some(value) => {
            Some(value.as_f64().filter(|millis| millis.is_finite()).ok_or_else(|| Error::other("Invalid time value"))?
                as i64)
        }
    };
    let options = crate::store::repair::RepairOptions {
        from_revision: input::flag(&request, "from_revision")?,
        backup_path: input::string(&request, "backup_path")?.map(PathBuf::from),
        now_millis,
    };
    let result = crate::store::repair::execute_confirmed_repair(call.store()?, &options)?;
    Ok(json::to_value(&result))
}
