//! `ath check` and `ath repair` against the 2.1.0 results.
//!
//! Each scenario in `tests/golden/maintenance/check-scenarios.json` names a
//! fixture and the SQL and file changes that damage it. The golden file holds
//! what the 2.1.0 TypeScript implementation reported. This test applies the
//! same changes with an independent SQLite connection and compares.

mod maintenance_support;

use std::path::{Path, PathBuf};

use athleticstandard::json;
use athleticstandard::store::check::{check, check_database, CheckIssue, Severity};
use athleticstandard::store::migrate::{execute_confirmed_migration, MigrationOptions};
use athleticstandard::store::repair::{execute_confirmed_repair, repair_preview, RepairOptions};
use athleticstandard::store::{OpenOptions, Store};
use maintenance_support::*;
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    match name {
        "demo" => demo_fixture(),
        "npm100" => npm100_fixture(),
        "v200" => version_200_fixture(),
        other => panic!("unknown fixture {other}"),
    }
}

/// Copy a fixture and apply a scenario's changes without the library.
fn damaged_copy(scenario: &Value) -> PathBuf {
    let fixture_name = scenario["fixture"].as_str().unwrap();
    let path = copy_fixture(&fixture(fixture_name), "ath-check-", fixture_name != "npm100");
    let dir = path.parent().unwrap().to_path_buf();
    if fixture_name != "npm100" {
        // Migrate first, so the schema step finding does not shift the issue
        // list. Migrating these fixtures changes no athlete row.
        let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
        let options = MigrationOptions { backup_path: Some(dir.join("pre-migration.sqlite")), now_millis: None };
        execute_confirmed_migration(&mut store, &options).unwrap();
        store.close().unwrap();
        std::fs::remove_file(dir.join("pre-migration.sqlite")).unwrap();
    }
    let connection = direct(&path);
    connection.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    for statement in scenario["sql"].as_array().unwrap() {
        connection.execute_batch(statement.as_str().unwrap()).unwrap();
    }
    drop(connection);
    for file in scenario.get("remove_files").and_then(Value::as_array).into_iter().flatten() {
        std::fs::remove_file(dir.join(file.as_str().unwrap())).unwrap();
    }
    if let Some(files) = scenario.get("write_files").and_then(Value::as_object) {
        for (file, text) in files {
            std::fs::write(dir.join(file), text.as_str().unwrap()).unwrap();
        }
    }
    path
}

/// Remove differences that 2.1.1 changes on purpose:
/// - the expected schema step is now 5, so the schema step finding differs;
/// - a missing attachment names the temporary folder and the operating
///   system's error text, which Node and Rust word differently.
fn comparable(issues: &[Value], dir: &Path) -> Vec<Value> {
    issues
        .iter()
        .filter(|issue| {
            !(issue["check"] == "schema_version"
                && issue["message"].as_str().unwrap().starts_with("database metadata schema version is"))
        })
        .map(|issue| {
            let mut issue = issue.clone();
            let message = issue["message"].as_str().unwrap().to_string();
            if let Some(rest) = message.strip_prefix("referenced attachment is missing: ") {
                let path = rest.split(" (").next().unwrap();
                let name = Path::new(path).file_name().unwrap().to_string_lossy().into_owned();
                issue["message"] = Value::String(format!("referenced attachment is missing: <dir>/attachments/{name}"));
            } else {
                let _ = dir;
            }
            issue
        })
        .collect()
}

fn issue_values(issues: &[CheckIssue]) -> Vec<Value> {
    issues.iter().map(|issue| serde_json::to_value(issue).unwrap()).collect()
}

#[test]
fn check_and_repair_match_2_1_0_on_every_scenario() {
    let golden = golden("check-scenarios.json");
    let mut failures = Vec::new();
    for scenario in golden["scenarios"].as_array().unwrap() {
        let name = scenario["name"].as_str().unwrap();
        let path = damaged_copy(scenario);
        let dir = path.parent().unwrap().to_path_buf();
        let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
        let result = check_database(&store).unwrap();
        let expected = &scenario["check"];

        let ours = comparable(&issue_values(&result.issues), &dir);
        let theirs = comparable(expected["issues"].as_array().unwrap(), &dir);
        if ours != theirs {
            failures.push(format!(
                "{name}: issues differ\n  2.1.0: {}\n  2.1.1: {}",
                json::to_string(&Value::Array(theirs)),
                json::to_string(&Value::Array(ours))
            ));
        }
        let mut expected_metadata = expected["metadata"].clone();
        if scenario["fixture"] != "npm100" {
            // These copies were migrated to 2.1.1 before the damage.
            expected_metadata["sql_schema_version"] = Value::from(5);
            expected_metadata["athleticstandard_version"] = Value::from("2.1.1");
        }
        if serde_json::to_value(&result.metadata).unwrap() != expected_metadata {
            failures.push(format!("{name}: metadata differs"));
        }
        if result.record_count != expected["record_count"] {
            failures.push(format!("{name}: record_count differs"));
        }
        // Only the schema step finding can make a 2.1.1 result invalid where 2.1.0 was valid.
        let other_errors =
            result.issues.iter().any(|issue| issue.severity == Severity::Error && issue.check != "schema_version");
        let old_other_errors = expected["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["severity"] == "error" && issue["check"] != "schema_version");
        if other_errors != old_other_errors {
            failures.push(format!("{name}: validity differs"));
        }

        let preview = repair_preview(&store).unwrap();
        let repair = &scenario["repair"];
        if preview.repair_candidates != repair["repair_candidates"]
            || preview.revision_align_candidates != repair["revision_align_candidates"]
            || serde_json::to_value(&preview.revision_align_record_ids).unwrap() != repair["revision_align_record_ids"]
        {
            failures.push(format!("{name}: repair preview {preview:?} differs from {repair}"));
        }
        drop(store);

        if let Some(repaired) = scenario.get("repaired") {
            let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
            let revisions_before = dump_tables(&path, &["record_revisions"]);
            let result = execute_confirmed_repair(
                &mut store,
                &RepairOptions { from_revision: true, ..RepairOptions::default() },
            )
            .unwrap();
            store.close().unwrap();
            if !result.backup_path.is_empty() != repaired["made_backup"]
                || result.repaired_record_count != repaired["repaired_record_count"]
                || result.revision_align_record_count != repaired["revision_align_record_count"]
                || result.database_revision != repaired["database_revision"]
            {
                failures.push(format!("{name}: repair result {result:?} differs from {repaired}"));
            }
            // A storage repair never adds a revision.
            assert_eq!(dump_tables(&path, &["record_revisions"]), revisions_before, "{name}");
            let tables = dump_tables(&path, ATHLETE_TABLES);
            for (table, rows) in &tables {
                let expected = &repaired["tables"][table];
                if rows.len() != expected["rows"] || table_hash(rows) != expected["sha256"] {
                    failures.push(format!("{name}: table {table} differs after repair"));
                }
            }
            let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
            let after = check_database(&store).unwrap();
            let errors: Vec<&CheckIssue> =
                after.issues.iter().filter(|issue| issue.check != "schema_version").collect();
            if !errors.is_empty() {
                failures.push(format!("{name}: check after repair still reports {errors:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn check_reads_without_changing_the_file() {
    let path = copy_fixture(&demo_fixture(), "ath-check-read-only-", true);
    direct(&path).execute_batch("PRAGMA journal_mode = DELETE").unwrap();
    let before = file_hash(&path);
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let report = check(&store, None).unwrap();
    drop(store);
    assert_eq!(file_hash(&path), before);
    assert_eq!(journal_mode(&path), "delete");
    assert_eq!(report.file, path.display().to_string());
    assert!(!report.export_validated);
    // The 2.1.0 demo is one step behind.
    assert_eq!(report.result.issues.len(), 1);
    assert_eq!(report.result.issues[0].message, "database metadata schema version is 4, expected 5");
    let value = serde_json::to_value(&report).unwrap();
    let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["file", "metadata", "valid", "record_count", "issues", "export_validated"]);
    let issue_keys: Vec<&String> = value["issues"][0].as_object().unwrap().keys().collect();
    assert_eq!(issue_keys, ["severity", "check", "message"]);
}

#[test]
fn the_export_hook_runs_after_the_check_and_its_error_stops_the_command() {
    let path = copy_fixture(&demo_fixture(), "ath-check-export-", true);
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let ran = std::cell::Cell::new(false);
    let report = check(
        &store,
        Some(&|_store: &Store| {
            ran.set(true);
            Ok(())
        }),
    )
    .unwrap();
    assert!(ran.get());
    assert!(report.export_validated);
    let error =
        check(&store, Some(&|_store: &Store| Err(athleticstandard::Error::other("package refused")))).unwrap_err();
    assert_eq!(error.to_string(), "package refused");
}

#[test]
fn pending_attachment_work_is_a_warning() {
    let path = copy_fixture(&demo_fixture(), "ath-check-pending-", true);
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    athleticstandard::store::migrate::execute_confirmed_migration(&mut store, &Default::default()).unwrap();
    store.close().unwrap();
    direct(&path)
        .execute(
            "INSERT INTO internal_pending_file_operations (operation_id, operation_kind, relative_path, created_at)
             VALUES ('01890f3e-e7d3-7cc0-88c8-0000000000aa', 'attachment_removal', 'attachments/x.png', '2026-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let result = check_database(&store).unwrap();
    assert!(result.valid, "{:?}", result.issues);
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].severity, Severity::Warning);
    assert_eq!(result.issues[0].check, "pending_file_operation");
    assert!(result.issues[0].message.contains("attachments/x.png"));
}

#[test]
fn every_typed_column_disagreement_is_found_and_repaired() {
    // One case per column the encoder writes, for every record in the demo.
    let base = copy_fixture(&demo_fixture(), "ath-check-columns-base-", true);
    let store = Store::open(&base, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let mut cases = Vec::new();
    for record in store
        .list_records(&athleticstandard::store::read::ListOptions {
            include_deleted: false,
            limit: 1000,
            ..Default::default()
        })
        .unwrap()
    {
        let table = athleticstandard::store::encode::table_for(record.record_type());
        for (column, _) in athleticstandard::store::encode::typed_columns(&record.data) {
            cases.push((record.record_id.clone(), table, column));
        }
    }
    drop(store);
    assert!(cases.len() > 80, "{}", cases.len());

    let mut missed = Vec::new();
    for (record_id, table, column) in cases {
        let path = copy_fixture(&demo_fixture(), "ath-check-column-", true);
        let connection = direct(&path);
        connection.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        let current: rusqlite::types::Value = connection
            .query_row(&format!("SELECT {column} FROM {table} WHERE record_id = ?1"), [&record_id], |row| row.get(0))
            .unwrap();
        let declared: String = connection
            .query_row(&format!("SELECT type FROM pragma_table_info('{table}') WHERE name = ?1"), [column], |row| {
                row.get(0)
            })
            .unwrap();
        use rusqlite::types::Value as Sql;
        let candidates: Vec<Sql> = match (&current, declared.as_str()) {
            (Sql::Integer(0), _) => vec![Sql::Integer(1)],
            (Sql::Integer(1), _) => vec![Sql::Integer(0)],
            (Sql::Integer(number), _) => vec![Sql::Integer(number + 1)],
            (Sql::Real(number), _) => vec![Sql::Real(number + 1.5), Sql::Real(number / 2.0), Sql::Real(0.0)],
            (Sql::Text(text), _) => {
                vec![Sql::Text(format!("{text}-damaged")), Sql::Text("wearable".into()), Sql::Text("manual".into())]
            }
            (Sql::Null, "INTEGER") => vec![Sql::Integer(7)],
            (Sql::Null, "REAL") => vec![Sql::Real(7.5)],
            (Sql::Null, _) => vec![Sql::Text("damaged".into())],
            (Sql::Blob(_), _) => vec![],
        };
        let written = candidates.into_iter().any(|value| {
            value != current
                && connection
                    .execute(
                        &format!("UPDATE {table} SET {column} = ?1 WHERE record_id = ?2"),
                        rusqlite::params![value, record_id],
                    )
                    .is_ok()
        });
        assert!(written, "{table}.{column} could not be damaged");
        drop(connection);

        let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
        let result = check_database(&store).unwrap();
        drop(store);
        let found = result
            .issues
            .iter()
            .any(|issue| issue.check == "typed_denormalized" && issue.record_id.as_deref() == Some(record_id.as_str()));
        if !found {
            missed.push(format!("{table}.{column}"));
            continue;
        }
        let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
        let repaired = execute_confirmed_repair(&mut store, &RepairOptions::default()).unwrap();
        assert_eq!(repaired.repaired_record_count, 1, "{table}.{column}");
        store.close().unwrap();
        let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
        let after = check_database(&store).unwrap();
        let errors: Vec<_> = after.issues.iter().filter(|issue| issue.check != "schema_version").collect();
        assert!(errors.is_empty(), "{table}.{column}: {errors:?}");
    }
    assert!(missed.is_empty(), "check missed damage in {missed:?}");
}

#[test]
fn a_meaning_difference_needs_from_revision() {
    let golden = golden("check-scenarios.json");
    let scenario = golden["scenarios"].as_array().unwrap().iter().find(|s| s["name"] == "data_json_meaning").unwrap();
    let path = damaged_copy(scenario);
    let before = file_hash(&path);
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let error = execute_confirmed_repair(&mut store, &RepairOptions::default()).unwrap_err().to_string();
    assert_eq!(
        error,
        "1 record(s) have typed-table bytes that disagree with the current revision in meaning (01890f3e-e7d3-7cc0-88c8-000000000007). Pass --from-revision and --confirm-revision-align with the database ID after reviewing the preview."
    );
    drop(store);
    assert_eq!(file_hash(&path), before);
    let dir = path.parent().unwrap();
    assert!(directory_names(dir).iter().all(|name| !name.contains("backup")), "{:?}", directory_names(dir));
}

#[test]
fn a_repair_backup_uses_the_2_1_0_name() {
    let golden = golden("check-scenarios.json");
    let scenario = golden["scenarios"].as_array().unwrap().iter().find(|s| s["name"] == "typed_column_value").unwrap();
    let path = damaged_copy(scenario);
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let result = execute_confirmed_repair(
        &mut store,
        &RepairOptions { now_millis: Some(1_768_478_400_123), ..RepairOptions::default() },
    )
    .unwrap();
    assert!(result.backup_path.starts_with(&format!("{}.repair-backup.20260115T120000.123Z-", path.display())));
    assert_eq!(file_mode(Path::new(&result.backup_path)), 0o600);
}

/// `repair --from-revision` rebuilds a typed row from the retained revision
/// without reading the damaged typed JSON. The npm 1.0.0 database is
/// repaired, migrated, damaged, and repaired again.
#[test]
fn from_revision_rebuilds_without_reading_the_damaged_row() {
    let path = copy_fixture(&npm100_fixture(), "ath-repair-from-revision-", false);
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let revision_before = store.database_revision().unwrap();
    let repaired = execute_confirmed_repair(&mut store, &RepairOptions::default()).unwrap();
    assert_eq!(repaired.repaired_record_count, 110);
    assert_eq!(repaired.database_revision, revision_before + 1);
    let migrated = execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    assert!(migrated.migrated);
    // Step 2 found nothing left to repair, so migration did not advance the revision.
    assert_eq!(store.database_revision().unwrap(), revision_before + 1);
    store.close().unwrap();

    let record_id: String = inspect(&path)
        .query_row(
            "SELECT record_id FROM point_measurements WHERE quantity = 'hrv_rmssd' ORDER BY record_id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    direct(&path)
        .execute(
            "UPDATE point_measurements SET data_json = '{\"x\":1}', standard_value = 999 WHERE record_id = ?1",
            [&record_id],
        )
        .unwrap();
    let store = Store::open(&path, OpenOptions::default()).unwrap();
    let preview = repair_preview(&store).unwrap();
    assert_eq!(preview.revision_align_record_ids, vec![record_id.clone()]);
    assert!(!check_database(&store).unwrap().valid);
    drop(store);

    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    let revisions_before = dump_tables(&path, &["record_revisions"]);
    let result =
        execute_confirmed_repair(&mut store, &RepairOptions { from_revision: true, ..RepairOptions::default() })
            .unwrap();
    assert_eq!(result.revision_align_record_count, 1);
    assert_eq!(dump_tables(&path, &["record_revisions"]), revisions_before);
    let after = check_database(&store).unwrap();
    assert!(after.valid && after.issues.is_empty(), "{:?}", after.issues);
}
