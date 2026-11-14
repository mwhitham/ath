//! Confirmed migration: preview without change, verified backup, one
//! transaction under the write lock, rollback, and refusals.

mod maintenance_support;

use std::path::{Path, PathBuf};

use athleticstandard::store::backup;
use athleticstandard::store::check::check_database;
use athleticstandard::store::contract;
use athleticstandard::store::migrate::{
    execute_confirmed_migration, inspect_migration, migrate_with_verified_backup, MigrationOptions,
};
use athleticstandard::store::{CreateOptions, OpenOptions, Store, SQL_SCHEMA_VERSION};
use athleticstandard::Error;
use maintenance_support::*;

fn open_for_migration(path: &Path) -> Store {
    Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap()
}

struct Case {
    name: &'static str,
    fixture: PathBuf,
    source_version: u32,
    source_software: &'static str,
    revision_bump: i64,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "1.0.0",
            fixture: npm100_fixture(),
            source_version: 1,
            source_software: "1.0.0",
            revision_bump: 1,
        },
        Case {
            name: "2.0.0",
            fixture: version_200_fixture(),
            source_version: 3,
            source_software: "2.0.0",
            revision_bump: 0,
        },
        Case { name: "2.1.0", fixture: demo_fixture(), source_version: 4, source_software: "2.1.0", revision_bump: 0 },
    ]
}

#[test]
fn every_supported_version_migrates_and_keeps_its_history() {
    let fresh_dir = temporary_dir("ath-migrate-fresh-");
    let fresh = fresh_dir.join("athlete.ath.sqlite");
    Store::create(&fresh, CreateOptions::default()).unwrap().close().unwrap();
    let fresh_schema = schema_objects(&fresh);
    let published = contract::published_contract().unwrap();

    for case in cases() {
        let path = copy_fixture(&case.fixture, "ath-migrate-", case.name != "1.0.0");
        let history_before = dump_tables(&path, HISTORY_TABLES);
        let athlete_before = dump_tables(&path, ATHLETE_TABLES);
        let revision_before = database_revision(&path);
        let main_before = file_hash(&path);
        let mode_before = journal_mode(&path);

        // The preview changes nothing.
        let preview = inspect_migration(&path, Some(1_768_478_400_123)).unwrap();
        assert_eq!(file_hash(&path), main_before, "{}: preview changed the file", case.name);
        assert_eq!(journal_mode(&path), mode_before, "{}: preview changed the journal mode", case.name);
        assert!(preview.preview.migration_needed);
        assert!(!preview.write_confirmed);
        assert_eq!(preview.preview.source_sql_schema_version, case.source_version);
        assert_eq!(preview.preview.target_sql_schema_version, SQL_SCHEMA_VERSION);
        let steps: Vec<u32> = preview.preview.planned_steps.iter().map(|step| step.target_sql_schema_version).collect();
        assert_eq!(steps, ((case.source_version + 1)..=SQL_SCHEMA_VERSION).collect::<Vec<_>>());
        let planned = preview.preview.planned_backup_path.clone().unwrap();
        assert!(
            planned.starts_with(&format!("{}.migration-backup.20260115T120000.123Z-", path.display())),
            "{planned}"
        );

        // An ordinary open refuses and does not migrate.
        match Store::open(&path, OpenOptions::default()) {
            Err(Error::MigrationRequired(required)) => {
                assert_eq!(required.source_sql_schema_version, case.source_version)
            }
            other => panic!("{}: expected a migration refusal, got {other:?}", case.name),
        }
        assert_eq!(file_hash(&path), main_before);

        let mut store = open_for_migration(&path);
        let result = execute_confirmed_migration(
            &mut store,
            &MigrationOptions { backup_path: Some(PathBuf::from(&planned)), now_millis: None },
        )
        .unwrap();
        assert!(result.migrated);
        assert_eq!(result.backup_path.as_deref(), Some(planned.as_str()));
        assert_eq!(store.metadata().sql_schema_version, SQL_SCHEMA_VERSION);
        assert_eq!(store.metadata().athleticstandard_version, "2.1.1");
        store.close().unwrap();

        // The backup holds the database as it was.
        let backup_path = PathBuf::from(&planned);
        assert_eq!(file_mode(&backup_path), 0o600);
        assert_eq!(journal_mode(&backup_path), "delete");
        for suffix in ["-wal", "-shm", "-journal"] {
            assert!(!athleticstandard::files::sidecar(&backup_path, suffix).exists());
        }
        assert_eq!(dump_tables(&backup_path, ATHLETE_TABLES), athlete_before, "{}: backup differs", case.name);
        let backup_metadata = athleticstandard::store::read_metadata(&inspect(&backup_path)).unwrap();
        assert_eq!(backup_metadata.sql_schema_version, case.source_version);
        assert_eq!(backup_metadata.athleticstandard_version, case.source_software);
        assert_eq!(database_revision(&backup_path), revision_before);

        // The migrated file opens normally and keeps every retained revision.
        let reopened = Store::open(&path, OpenOptions::default()).unwrap();
        assert_eq!(reopened.metadata().sql_schema_version, SQL_SCHEMA_VERSION);
        let user_version: u32 =
            reopened.connection().pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(user_version, SQL_SCHEMA_VERSION);
        let report = check_database(&reopened).unwrap();
        assert!(report.valid, "{}: {:?}", case.name, report.issues);
        assert!(report.issues.is_empty(), "{}: {:?}", case.name, report.issues);
        assert_eq!(contract::verify_live_schema(reopened.connection(), &published).unwrap(), Vec::<String>::new());
        drop(reopened);
        assert_eq!(dump_tables(&path, HISTORY_TABLES), history_before, "{}: history changed", case.name);
        assert_eq!(database_revision(&path), revision_before + case.revision_bump, "{}", case.name);
        if case.revision_bump == 0 {
            assert_eq!(dump_tables(&path, ATHLETE_TABLES), athlete_before, "{}: athlete rows changed", case.name);
        }
        // 2.0.0 created `workout_sessions` with its three 2.0.0 columns before
        // `data_json`. Databases made by the migration chain add them after.
        // The column set is the same; the encoder names every column.
        let migrated = schema_objects(&path);
        let names = |objects: &[(String, String, Option<String>)]| -> Vec<(String, String)> {
            objects.iter().map(|(kind, name, _)| (kind.clone(), name.clone())).collect()
        };
        assert_eq!(names(&migrated), names(&fresh_schema), "{}: schema objects differ", case.name);
        for (object, fresh_object) in migrated.iter().zip(fresh_schema.iter()) {
            if case.name == "2.0.0" && object.1 == "workout_sessions" {
                let connection = inspect(&path);
                let mut migrated_columns: Vec<String> = connection
                    .prepare("SELECT name FROM pragma_table_info('workout_sessions')")
                    .unwrap()
                    .query_map([], |row| row.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                let mut fresh_columns: Vec<String> = inspect(&fresh)
                    .prepare("SELECT name FROM pragma_table_info('workout_sessions')")
                    .unwrap()
                    .query_map([], |row| row.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                migrated_columns.sort();
                fresh_columns.sort();
                assert_eq!(migrated_columns, fresh_columns);
                continue;
            }
            assert_eq!(object, fresh_object, "{}: schema differs from a new database", case.name);
        }
    }
}

#[test]
fn a_current_database_needs_no_migration_and_no_backup() {
    let dir = temporary_dir("ath-migrate-current-");
    let path = dir.join("athlete.ath.sqlite");
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    let result = execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    assert!(!result.migrated);
    assert_eq!(result.backup_path, None);
    assert!(!result.preview.migration_needed);
    assert!(result.preview.planned_steps.is_empty());
    assert_eq!(result.preview.planned_backup_path, None);
    assert_eq!(
        result.preview.compatibility_effect,
        "No migration is required. Direct-read views and writable tables match this ath version."
    );
    store.close().unwrap();
    assert_eq!(directory_names(&dir), vec!["athlete.ath.sqlite"]);
}

#[test]
fn a_failed_step_rolls_back_and_names_the_verified_backup() {
    // A typed row that differs from its revision in meaning stops step 2.
    let path = copy_fixture(&npm100_fixture(), "ath-migrate-divergent-", false);
    {
        let connection = direct(&path);
        connection.execute_batch("PRAGMA journal_mode = DELETE").unwrap();
        let record_id: String = connection
            .query_row(
                "SELECT record_id FROM point_measurements WHERE quantity = 'hrv_rmssd' ORDER BY record_id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE point_measurements SET data_json = json_set(data_json, '$.value.standard_value', 999) WHERE record_id = ?1",
                [&record_id],
            )
            .unwrap();
    }
    assert_eq!(journal_mode(&path), "delete");
    let before = dump_tables(&path, ATHLETE_TABLES);
    let revision_before = database_revision(&path);
    let backup_path = athleticstandard::files::sidecar(&path, ".reviewed-backup.sqlite");

    let mut store = open_for_migration(&path);
    let error = execute_confirmed_migration(
        &mut store,
        &MigrationOptions { backup_path: Some(backup_path.clone()), now_millis: None },
    )
    .unwrap_err()
    .to_string();
    drop(store);
    let expected_prefix =
        format!("Migration failed after creating verified backup at {}. Cause: record ", backup_path.display());
    assert!(error.starts_with(&expected_prefix), "{error}");
    assert!(
        error.ends_with(
            "has a typed-table mismatch that is not key-order only; run ath repair --from-revision after reviewing affected records, or restore from backup. The database revision did not change."
        ),
        "{error}"
    );
    assert!(!error.contains("SQLITE_BUSY"));
    // Nothing changed: same rows, same schema step, same journal mode.
    assert_eq!(dump_tables(&path, ATHLETE_TABLES), before);
    assert_eq!(database_revision(&path), revision_before);
    let metadata = athleticstandard::store::read_metadata(&inspect(&path)).unwrap();
    assert_eq!(metadata.sql_schema_version, 1);
    assert_eq!(journal_mode(&path), "delete");
    // The verified backup stays.
    assert_eq!(dump_tables(&backup_path, ATHLETE_TABLES), before);
}

#[test]
fn a_change_after_the_backup_makes_it_stale() {
    let path = copy_fixture(&version_200_fixture(), "ath-migrate-stale-", false);
    let mut store = open_for_migration(&path);
    let backup_path = athleticstandard::files::sidecar(&path, ".early-backup.sqlite");
    let verified = store.create_verified_backup(&backup_path).unwrap();
    assert_eq!(verified.database_revision as i64, database_revision(&path));

    // Another writer changes the database after the backup.
    direct(&path)
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();

    let error = migrate_with_verified_backup(&mut store, &verified).unwrap_err();
    assert!(matches!(error, Error::Stale(_)), "{error:?}");
    let message = error.to_string();
    assert!(message.starts_with("Database revision changed before migration could start. Nothing was migrated."));
    assert!(message.contains(&backup_path.display().to_string()));
    drop(store);
    let metadata = athleticstandard::store::read_metadata(&inspect(&path)).unwrap();
    assert_eq!(metadata.sql_schema_version, 3);

    // A new backup of the current state works.
    let mut store = open_for_migration(&path);
    let fresh_backup =
        store.create_verified_backup(&athleticstandard::files::sidecar(&path, ".new-backup.sqlite")).unwrap();
    let result = migrate_with_verified_backup(&mut store, &fresh_backup).unwrap();
    assert!(result.migrated);
}

#[test]
fn migration_uses_the_reviewed_backup_path() {
    let path = copy_fixture(&version_200_fixture(), "ath-migrate-reviewed-", false);
    let reviewed = inspect_migration(&path, None).unwrap().preview.planned_backup_path.unwrap();
    let regenerated = inspect_migration(&path, None).unwrap().preview.planned_backup_path.unwrap();
    assert_ne!(reviewed, regenerated);
    let mut store = open_for_migration(&path);
    let result = execute_confirmed_migration(
        &mut store,
        &MigrationOptions { backup_path: Some(PathBuf::from(&reviewed)), now_millis: None },
    )
    .unwrap();
    assert_eq!(result.backup_path.as_deref(), Some(reviewed.as_str()));
    assert!(Path::new(&reviewed).is_file());
    assert!(!Path::new(&regenerated).exists());
}

#[test]
fn foreign_and_mismatched_headers_are_refused_without_change() {
    // A foreign application ID.
    let path = copy_fixture(&demo_fixture(), "ath-migrate-foreign-", false);
    {
        let connection = direct(&path);
        connection.execute_batch("PRAGMA journal_mode = DELETE; PRAGMA application_id = 1234;").unwrap();
    }
    let before = file_hash(&path);
    let error = inspect_migration(&path, None).unwrap_err().to_string();
    assert_eq!(error, "SQLite application ID 1234 does not belong to Athletic Standard");
    let error = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap_err();
    assert_eq!(error.to_string(), "SQLite application ID 1234 does not belong to Athletic Standard");
    assert_eq!(file_hash(&path), before);
    assert_eq!(journal_mode(&path), "delete");

    // A user_version that disagrees with metadata.
    let path = copy_fixture(&demo_fixture(), "ath-migrate-user-version-", false);
    direct(&path).execute_batch("PRAGMA journal_mode = DELETE; PRAGMA user_version = 3;").unwrap();
    let before = file_hash(&path);
    let error = inspect_migration(&path, None).unwrap_err().to_string();
    assert_eq!(error, "SQLite user_version 3 does not match database metadata 4");
    assert_eq!(file_hash(&path), before);

    // A newer schema than this program writes.
    let path = copy_fixture(&demo_fixture(), "ath-migrate-newer-", false);
    direct(&path)
        .execute_batch(
            "PRAGMA journal_mode = DELETE;
             UPDATE database_metadata SET sql_schema_version = 6;
             PRAGMA user_version = 6;",
        )
        .unwrap();
    let error = inspect_migration(&path, None).unwrap_err().to_string();
    assert_eq!(error, "Database SQL schema 6 is newer than supported schema 5");
}

#[test]
fn rollback_means_restoring_the_verified_backup() {
    let path = copy_fixture(&demo_fixture(), "ath-migrate-rollback-", true);
    let before = dump_tables(&path, ATHLETE_TABLES);
    let mut store = open_for_migration(&path);
    let result = execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    store.close().unwrap();
    let backup_path = PathBuf::from(result.backup_path.unwrap());

    // Keep the newer database separately, then restore the backup in its place.
    let kept = athleticstandard::files::sidecar(&path, ".2.1.1.sqlite");
    std::fs::rename(&path, &kept).unwrap();
    for suffix in ["-wal", "-shm"] {
        let sidecar = athleticstandard::files::sidecar(&path, suffix);
        if sidecar.exists() {
            std::fs::rename(&sidecar, athleticstandard::files::sidecar(&kept, suffix)).unwrap();
        }
    }
    std::fs::copy(&backup_path, &path).unwrap();
    assert_eq!(dump_tables(&path, ATHLETE_TABLES), before);
    let metadata = athleticstandard::store::read_metadata(&inspect(&path)).unwrap();
    assert_eq!(metadata.athleticstandard_version, "2.1.0");
    assert_eq!(metadata.sql_schema_version, 4);
    let kept_metadata = athleticstandard::store::read_metadata(&inspect(&kept)).unwrap();
    assert_eq!(kept_metadata.athleticstandard_version, "2.1.1");
    assert!(backup::format_migration_backup_timestamp(0).ends_with('Z'));
}

/// Runs the 2.1.0 TypeScript `ath` from a reference checkout when
/// `ATH_REFERENCE_CHECKOUT` names one with its `node_modules`. The old
/// program must refuse a migrated 2.1.1 database and leave it unchanged. It
/// must read the restored backup.
#[test]
fn the_2_1_0_program_refuses_a_migrated_database() {
    let Ok(checkout) = std::env::var("ATH_REFERENCE_CHECKOUT") else {
        eprintln!("skipped: set ATH_REFERENCE_CHECKOUT to a 2.1.0 checkout with node_modules");
        return;
    };
    let checkout = PathBuf::from(checkout);
    let path = copy_fixture(&demo_fixture(), "ath-migrate-old-cli-", true);
    let dir = path.parent().unwrap().to_path_buf();
    let mut store = open_for_migration(&path);
    let result = execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    store.close().unwrap();
    let backup_path = PathBuf::from(result.backup_path.unwrap());
    let before = file_hash(&path);

    let old = |args: &[&str]| {
        let output = std::process::Command::new("node")
            .arg("--import")
            .arg("tsx")
            .arg(checkout.join("src/cli.ts"))
            .args(args)
            .current_dir(&checkout)
            .output()
            .expect("node runs");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr),
        )
    };
    let file = path.display().to_string();
    for args in [
        vec!["stats", "--file", file.as_str()],
        vec!["check", "--file", file.as_str()],
        vec!["records", "list", "--file", file.as_str()],
        vec!["migrate", "--json", "--file", file.as_str()],
        vec!["log", "--file", file.as_str(), "--yes", "felt fine"],
    ] {
        let (code, output) = old(&args);
        assert_eq!(code, Some(1), "{args:?}: {output}");
        assert!(output.contains("Unsupported Athletic Standard database version: 2.1.1"), "{args:?}: {output}");
    }
    assert_eq!(file_hash(&path), before, "the old program changed the migrated database");

    // After restoring the backup, the old program reads it again.
    let restored = dir.join("restored.ath.sqlite");
    std::fs::copy(&backup_path, &restored).unwrap();
    let (code, output) = old(&["stats", "--json", "--file", restored.to_str().unwrap()]);
    assert_eq!(code, Some(0), "{output}");
    assert!(output.contains("\"athleticstandard_version\": \"2.1.0\""), "{output}");
}

/// The migrated athlete tables equal what the 2.1.0 migration wrote, row for
/// row. 2.1.1 adds step 5, which changes no athlete row.
#[test]
fn migrated_tables_match_the_2_1_0_migration() {
    let golden = golden("migration.json");
    for expected in golden["fixtures"].as_array().unwrap() {
        let name = expected["fixture"].as_str().unwrap();
        let fixture = match name {
            "npm100" => npm100_fixture(),
            "v200" => version_200_fixture(),
            "demo" => demo_fixture(),
            other => panic!("unknown fixture {other}"),
        };
        let path = copy_fixture(&fixture, "ath-migrate-golden-", false);
        let preview = inspect_migration(&path, None).unwrap().preview;
        let old_preview = &expected["preview"];
        assert_eq!(preview.database_id, old_preview["database_id"].as_str().unwrap());
        assert_eq!(
            serde_json::Value::from(preview.source_sql_schema_version),
            old_preview["source_sql_schema_version"]
        );
        // 2.1.0 planned the same steps, ending one step earlier.
        let old_steps = old_preview["planned_steps"].as_array().unwrap();
        for (ours, theirs) in preview.planned_steps.iter().zip(old_steps) {
            assert_eq!(serde_json::to_value(ours).unwrap(), *theirs, "{name}");
        }
        assert_eq!(preview.planned_steps.len(), old_steps.len() + 1, "{name}");
        assert_eq!(preview.planned_steps.last().unwrap().migration_file, "migrations/005-version-2-1-1.sql");

        let mut store = open_for_migration(&path);
        execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
        store.close().unwrap();
        let tables = dump_tables(&path, ATHLETE_TABLES);
        for (table, rows) in &tables {
            let old = &expected["tables"][table];
            assert_eq!(serde_json::Value::from(rows.len()), old["rows"], "{name}: {table} row count");
            assert_eq!(serde_json::Value::from(table_hash(rows)), old["sha256"], "{name}: {table} rows differ");
        }
        assert_eq!(
            database_revision(&path).to_string(),
            expected["tables"]["database_state"]["sha256"].as_str().unwrap()
        );
    }
}
