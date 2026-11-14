//! Operations that need no database: IDs, validation, names, and formatting.

use std::path::Path;

use crate::error::Error;
use crate::json::{self, Value};
use crate::protocol::definition::{Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};
use crate::store::backup;

use super::input;

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "ids.uuid_v7",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ now?: number | string }",
        output: "string",
        items: None,
        callbacks: &[],
        run: uuid_v7,
    },
    Operation {
        name: "model.validate",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ schema: string; json_text: string }",
        output: "{ valid: boolean; issues: Array<{ path: Array<string | number>; code: string; message: string }>; output_text: string | null }",
        items: None,
        callbacks: &[],
        run: validate,
    },
    Operation {
        name: "model.parse",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ schema: string; value: unknown }",
        output: "unknown",
        items: None,
        callbacks: &[],
        run: parse,
    },
    Operation {
        name: "migration.needs_migration",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ metadata: unknown }",
        output: "boolean",
        items: None,
        callbacks: &[],
        run: needs_migration,
    },
    Operation {
        name: "migration.preview",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ metadata: unknown; local_path?: string; now_iso: string; sequence: number }",
        output: "Api.MigrationPreview",
        items: None,
        callbacks: &[],
        run: migration_preview,
    },
    Operation {
        name: "backup.timestamp",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ iso: string }",
        output: "string",
        items: None,
        callbacks: &[],
        run: backup_timestamp,
    },
    Operation {
        name: "backup.default_path",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ database_path: string; now_iso: string; sequence: number }",
        output: "string",
        items: None,
        callbacks: &[],
        run: backup_default_path,
    },
    Operation {
        name: "backup.sqlite3_command",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ backup_path: string }",
        output: "string",
        items: None,
        callbacks: &[],
        run: sqlite3_command,
    },
    Operation {
        name: "calendar.date_in_zone",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ timestamp: string; time_zone: string }",
        output: "string",
        items: None,
        callbacks: &[],
        run: calendar_date,
    },
    Operation {
        name: "json.deterministic",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ value: unknown }",
        output: "string",
        items: None,
        callbacks: &[],
        run: deterministic_json,
    },
];

/// The largest timestamp a UUIDv7 holds: 48 bits of milliseconds.
const MAX_UUID_MILLIS: f64 = 281_474_976_710_655.0;

/// `createUuidV7(now?)`.
fn uuid_v7(call: &mut Call<'_>) -> Outcome {
    let Some(now) = input::get(&call.input, "now") else {
        return Ok(Value::String(crate::ids::new_record_id()));
    };
    let millis = now
        .as_f64()
        .filter(|millis| millis.fract() == 0.0 && *millis >= 0.0 && *millis <= MAX_UUID_MILLIS)
        .ok_or_else(|| Error::other("UUIDv7 timestamp is outside its 48-bit range"))?;
    let millis = millis as u64;
    let timestamp = uuid::Timestamp::from_unix(uuid::NoContext, millis / 1000, ((millis % 1000) * 1_000_000) as u32);
    Ok(Value::String(uuid::Uuid::new_v7(timestamp).hyphenated().to_string()))
}

/// Validate JSON text with a published shape, for the compatibility tests.
fn validate(call: &mut Call<'_>) -> Outcome {
    let name = input::required_string(&call.input, "schema")?;
    let text = input::required_string(&call.input, "json_text")?;
    let schema = crate::model::registry::shape(&name)
        .ok_or_else(|| Failure::Input(format!("no published shape named {name}")))?;
    let value = json::parse(&text).map_err(|error| Error::other(format!("not valid JSON: {error}")))?;
    let issues = schema.issues(&value);
    let valid = issues.is_empty();
    let output_text = if valid { Value::String(json::to_string(&schema.output(&value))) } else { Value::Null };
    Ok(serde_json::json!({
        "valid": valid,
        "issues": issues.iter().map(|issue| issue.to_json()).collect::<Vec<_>>(),
        "output_text": output_text,
    }))
}

/// Check one argument with a published shape, as a 2.1.0 `Schema.parse` did
/// before any other work.
fn parse(call: &mut Call<'_>) -> Outcome {
    let name = input::required_string(&call.input, "schema")?;
    if crate::model::registry::shape(&name).is_none() {
        return Err(Failure::Input(format!("no published shape named {name}")));
    }
    input::check(&name, call.input.get("value").unwrap_or(&Value::Null))
}

fn sql_schema_version(metadata: &Value) -> Option<f64> {
    metadata.get("sql_schema_version").and_then(Value::as_f64)
}

/// `databaseNeedsMigration(metadata)`: the step is older than this build's.
fn needs_migration(call: &mut Call<'_>) -> Outcome {
    let metadata = input::required(&call.input, "metadata")?;
    let needed =
        sql_schema_version(metadata).is_some_and(|version| version < f64::from(crate::store::SQL_SCHEMA_VERSION));
    Ok(Value::Bool(needed))
}

/// `buildMigrationPreview(metadata, localPath?, now?)`.
fn migration_preview(call: &mut Call<'_>) -> Outcome {
    let metadata = input::required(&call.input, "metadata")?;
    let version = sql_schema_version(metadata)
        .filter(|version| version.fract() == 0.0 && *version >= 0.0)
        .ok_or_else(|| Failure::Input("metadata.sql_schema_version must be a whole number".into()))?;
    let database_id = metadata.get("database_id").and_then(Value::as_str).unwrap_or_default();
    let planned_backup_path = match input::string(&call.input, "local_path")? {
        Some(path) => Some(default_path(&call.input, &path)?),
        None => None,
    };
    let preview = crate::store::migrate::preview(version as u32, database_id, planned_backup_path);
    Ok(json::to_value(&preview))
}

/// `formatMigrationBackupTimestamp(date)`, from the date's ISO text.
fn backup_timestamp(call: &mut Call<'_>) -> Outcome {
    let iso = input::required_string(&call.input, "iso")?;
    Ok(Value::String(backup::format_iso_backup_timestamp(&iso)))
}

fn default_path(input: &Value, database_path: &str) -> Result<String, Failure> {
    let iso = input::required_string(input, "now_iso")?;
    let sequence = input::get(input, "sequence")
        .and_then(input::whole_number)
        .ok_or_else(|| Failure::Input("input.sequence must be a whole number".into()))?;
    Ok(backup::migration_backup_path(Path::new(database_path), &iso, sequence as u32).display().to_string())
}

/// `defaultMigrationBackupPath(databasePath, now?)`. The client keeps the
/// name counter, because each call runs in a new worker.
fn backup_default_path(call: &mut Call<'_>) -> Outcome {
    let database_path = input::required_string(&call.input, "database_path")?;
    Ok(Value::String(default_path(&call.input, &database_path)?))
}

/// `sqlite3BackupDotCommand(backupPath)`: the 2.1.0 `.backup` line for the
/// sqlite3 program. Native backups use the SQLite library instead; the export
/// stays for callers that still run sqlite3 themselves.
fn sqlite3_command(call: &mut Call<'_>) -> Outcome {
    let path = input::required_string(&call.input, "backup_path")?;
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"").replace('\r', "\\r").replace('\n', "\\n");
    Ok(Value::String(format!(".backup \"{escaped}\"\n")))
}

/// `calendarDateInZone(timestamp, timeZone)`.
fn calendar_date(call: &mut Call<'_>) -> Outcome {
    let timestamp = input::required_string(&call.input, "timestamp")?;
    let zone = input::required_string(&call.input, "time_zone")?;
    Ok(Value::String(crate::view::calendar::calendar_date_in_zone(&timestamp, &zone)?))
}

/// `deterministicJson(value)`: sorted keys, two-space indentation, and a
/// final newline. The client returns the text as a Buffer.
fn deterministic_json(call: &mut Call<'_>) -> Outcome {
    let value = call.input.get("value").cloned().unwrap_or(Value::Null);
    Ok(Value::String(json::to_deterministic_pretty(&value)))
}
