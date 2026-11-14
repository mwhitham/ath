//! Shared helpers for the database maintenance tests: fixtures, independent
//! SQLite connections, table dumps, and file hashes.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// npm 1.0.0 provider database, SQL schema 1.
pub fn npm100_fixture() -> PathBuf {
    repository().join("tests/fixtures/npm-100-encoding/athlete.ath.sqlite")
}

/// Software 2.0.0 database, SQL schema 3.
pub fn version_200_fixture() -> PathBuf {
    repository().join("tests/fixtures/version-2-0-0/database.ath.sqlite")
}

/// The deterministic 2.1.0 demo database, SQL schema 4, every record type.
pub fn demo_fixture() -> PathBuf {
    repository().join("examples/demo-athlete/athlete.ath.sqlite")
}

pub fn temporary_dir(prefix: &str) -> PathBuf {
    athleticstandard::files::make_temporary_directory(prefix).unwrap()
}

/// Copy a fixture's main file into a new private folder as
/// `athlete.ath.sqlite`. With `attachments`, copy the demo attachments too.
pub fn copy_fixture(fixture: &Path, prefix: &str, attachments: bool) -> PathBuf {
    let dir = temporary_dir(prefix);
    let path = dir.join("athlete.ath.sqlite");
    std::fs::copy(fixture, &path).unwrap();
    athleticstandard::files::set_owner_only(&path).unwrap();
    if attachments {
        let beside = fixture.parent().unwrap().join("attachments");
        let source = if beside.is_dir() { beside } else { fixture.parent().unwrap().join("package/attachments") };
        let target = dir.join("attachments");
        std::fs::create_dir(&target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
    path
}

/// An independent read-write connection that follows the published recipe's
/// connection settings and does not use the library.
pub fn direct(path: &Path) -> Connection {
    let connection =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX).unwrap();
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;").unwrap();
    connection
}

/// An independent read-only connection.
pub fn inspect(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).unwrap()
}

pub fn journal_mode(path: &Path) -> String {
    // The header's read and write version bytes are 2 in WAL mode and 1 in
    // rollback mode. Reading them does not open a SQLite connection.
    let bytes = std::fs::read(path).unwrap();
    match (bytes[18], bytes[19]) {
        (2, 2) => "wal".to_string(),
        (1, 1) => "delete".to_string(),
        other => format!("{other:?}"),
    }
}

/// History tables: retained revisions, deletions, erasures, and import keys.
pub const HISTORY_TABLES: &[&str] =
    &["records", "record_revisions", "record_deletions", "record_erasure_markers", "record_import_keys"];

/// Every athlete-data table, including typed and child tables.
pub const ATHLETE_TABLES: &[&str] = &[
    "records",
    "record_revisions",
    "record_deletions",
    "record_erasure_markers",
    "record_import_keys",
    "athlete_profile",
    "sources",
    "source_devices",
    "point_measurements",
    "point_measurement_calculations",
    "point_measurement_calculation_inputs",
    "measurement_samples",
    "sleep_sessions",
    "workout_sessions",
    "workout_session_segments",
    "workout_route_points",
    "benchmark_results",
    "benchmark_result_segments",
    "provider_scores",
    "provider_summaries",
    "provider_reporting_periods",
    "athlete_reports",
    "nutrition_entries",
    "reported_nutrition_items",
    "attachments",
    "record_attachment_links",
    "benchmark_definitions",
    "predictions",
    "prediction_result_links",
    "prediction_grades",
    "prediction_error_analyses",
    "prediction_error_causes",
    "training_plans",
    "planned_sessions",
    "planned_session_blocks",
    "coaching_notes",
    "saved_training_reviews",
    "goals",
    "nutrition_model_interpretations",
    "authored_record_evidence",
];

fn json_cell(value: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value as Sql;
    match value {
        Sql::Null => serde_json::Value::Null,
        Sql::Integer(number) => serde_json::Value::from(number),
        Sql::Real(number) => serde_json::Value::from(number),
        Sql::Text(text) => serde_json::Value::String(text),
        Sql::Blob(bytes) => serde_json::Value::String(bytes.iter().map(|byte| format!("{byte:02x}")).collect()),
    }
}

/// Rows of a table as compact JSON arrays in column order, sorted. Numbers use
/// JavaScript formatting so the reference generator writes the same text.
pub fn dump_table(connection: &Connection, table: &str) -> Vec<String> {
    let exists: bool = connection
        .query_row("SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)", [table], |row| {
            row.get(0)
        })
        .unwrap();
    if !exists {
        return Vec::new();
    }
    let mut statement = connection.prepare(&format!("SELECT * FROM {table}")).unwrap();
    let count = statement.column_count();
    let mut rows: Vec<String> = statement
        .query_map([], |row| {
            let mut cells = Vec::with_capacity(count);
            for index in 0..count {
                cells.push(json_cell(row.get(index)?));
            }
            Ok(athleticstandard::json::to_string(&serde_json::Value::Array(cells)))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows.sort();
    rows
}

pub fn dump_tables(path: &Path, tables: &[&str]) -> BTreeMap<String, Vec<String>> {
    let connection = inspect(path);
    tables.iter().map(|table| (table.to_string(), dump_table(&connection, table))).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 of a table dump: each sorted row followed by a newline.
pub fn table_hash(rows: &[String]) -> String {
    let mut text = String::new();
    for row in rows {
        text.push_str(row);
        text.push('\n');
    }
    sha256_hex(text.as_bytes())
}

pub fn file_hash(path: &Path) -> String {
    sha256_hex(&std::fs::read(path).unwrap())
}

/// The schema objects as `(type, name, sql)`, sorted.
pub fn schema_objects(path: &Path) -> Vec<(String, String, Option<String>)> {
    let connection = inspect(path);
    let mut statement = connection.prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name").unwrap();
    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap().map(Result::unwrap).collect()
}

pub fn database_revision(path: &Path) -> i64 {
    inspect(path).query_row("SELECT current_revision FROM database_state", [], |row| row.get(0)).unwrap()
}

pub fn file_mode(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0o600
    }
}

pub fn directory_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Golden files written by `native/tools/reference/maintenance-golden.ts`.
pub fn golden(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/maintenance").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap()
}
