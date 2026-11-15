//! Failure injection for the confirmed migration, the provider import, and
//! the confirmed repair (spec section 13.2). `package::tests` does the same
//! for the package operations, with the same failure points.
//!
//! Each case runs the operation in a child test process that stops at a
//! named point at once, with no rollback or cleanup, as a killed process
//! would. The parent then opens the database again and checks it with an
//! independent SQLite connection: it holds all of the change or none of it,
//! a verified backup is intact, and a database left unchanged can run the
//! operation again. An armed point makes the operation fail there instead
//! and run its own error path, which must change nothing.
//!
//! `ath log` with an attachment has no case here. It writes its files with
//! `package::prepare_attachments` and `package::apply_with_attachments`,
//! which `package::tests` stops at every file and SQL boundary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::Result;
use crate::files;
use crate::import::{self, ImportOptions, ProviderImportResult};
use crate::package::failpoints::{self, STOP_STATUS, STOP_VARIABLE};
use crate::store::migrate::SQL_SCHEMA_VERSION;
use crate::store::migrate::{execute_confirmed_migration, ConfirmedMigrationResult, MigrationOptions};
use crate::store::repair::{execute_confirmed_repair, repair_preview, ConfirmedRepairResult, RepairOptions};
use crate::store::{self, CreateOptions, OpenOptions, Store};

const DIRECTORY_VARIABLE: &str = "ATH_TEST_DIRECTORY";
const DATABASE: &str = "athlete.ath.sqlite";
const BACKUP: &str = "backup.sqlite";
const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn temporary(prefix: &str) -> PathBuf {
    files::make_temporary_directory(prefix).unwrap()
}

fn copy_fixture(relative: &str, directory: &Path) -> PathBuf {
    let path = directory.join(DATABASE);
    fs::copy(repository().join(relative), &path).unwrap();
    files::set_owner_only(&path).unwrap();
    path
}

fn open_outdated(path: &Path) -> Store {
    Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap()
}

/// Run one ignored child test in a new process. With `point`, the child must
/// stop there; without it, the child must pass. `temporary` becomes the
/// child's system temporary folder.
fn run_child(test: &str, point: Option<&str>, directory: &Path, temporary: Option<&Path>) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([test, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(DIRECTORY_VARIABLE, directory)
        .env_remove(STOP_VARIABLE);
    if let Some(point) = point {
        command.env(STOP_VARIABLE, point);
    }
    if let Some(temporary) = temporary {
        command.env("TMPDIR", temporary).env("TMP", temporary).env("TEMP", temporary);
    }
    let status = command.status().unwrap();
    let expected = if point.is_some() { STOP_STATUS } else { 0 };
    assert_eq!(status.code(), Some(expected), "{test} at {point:?}");
}

fn child_directory() -> Option<PathBuf> {
    std::env::var_os(DIRECTORY_VARIABLE).map(PathBuf::from)
}

fn armed<T>(point: &str, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    failpoints::arm(point);
    let result = operation();
    failpoints::disarm_all();
    result
}

/// What an independent read-only SQLite connection sees in a database file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Facts {
    integrity: String,
    sql_schema_version: u32,
    user_version: u32,
    database_revision: u64,
    records: i64,
}

fn facts(path: &Path) -> Facts {
    let connection = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    Facts {
        integrity: connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap(),
        sql_schema_version: store::read_metadata(&connection).unwrap().sql_schema_version,
        user_version: connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap(),
        database_revision: store::database_revision(&connection).unwrap(),
        records: connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap(),
    }
}

// ---- Confirmed migration -------------------------------------------------------

/// The 2.0.0 database is at SQL schema step 3, so a migration applies steps
/// 4 and 5 in one transaction. Step 4 alone would be a database in between.
const MIGRATION_FIXTURE: &str = "tests/fixtures/version-2-0-0/database.ath.sqlite";

fn migrate(database: &Path, backup: &Path) -> Result<ConfirmedMigrationResult> {
    let mut store = Store::open(database, OpenOptions { read_only: false, allow_outdated_schema: true })?;
    execute_confirmed_migration(
        &mut store,
        &MigrationOptions { backup_path: Some(backup.to_path_buf()), now_millis: None },
    )
}

#[test]
#[ignore = "run by run_child"]
fn child_migration() {
    if let Some(directory) = child_directory() {
        migrate(&directory.join(DATABASE), &directory.join(BACKUP)).unwrap();
    }
}

#[test]
fn a_stopped_migration_is_at_its_first_or_last_step_and_its_backup_is_intact() {
    for (point, migrated) in
        [("migrate.after_backup", false), ("migrate.before_commit", false), ("migrate.after_commit", true)]
    {
        let directory = temporary("ath-stop-migrate-");
        let database = copy_fixture(MIGRATION_FIXTURE, &directory);
        let before = facts(&database);
        assert_eq!((before.sql_schema_version, before.user_version), (3, 3));
        run_child("stopped_writes::child_migration", Some(point), &directory, None);

        // This process opens the file the stopped one left.
        drop(open_outdated(&database));
        let after = facts(&database);
        assert_eq!(after.integrity, "ok", "{point}");
        assert_eq!(after.records, before.records, "{point}");
        if migrated {
            assert_eq!((after.sql_schema_version, after.user_version), (SQL_SCHEMA_VERSION, SQL_SCHEMA_VERSION));
        } else {
            assert_eq!(after, before, "{point}: the database must be at its first step");
        }

        // The verified backup is the starting database, and it is whole.
        assert_eq!(facts(&directory.join(BACKUP)), before, "{point}");

        if !migrated {
            // The unchanged database migrates again with a new backup.
            let result = migrate(&database, &directory.join("second-backup.sqlite")).unwrap();
            assert!(result.migrated, "{point}");
            assert_eq!(facts(&database).sql_schema_version, SQL_SCHEMA_VERSION, "{point}");
        }
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_failed_migration_changes_nothing_and_keeps_its_backup() {
    for point in ["migrate.after_backup", "migrate.before_commit"] {
        let directory = temporary("ath-fail-migrate-");
        let database = copy_fixture(MIGRATION_FIXTURE, &directory);
        let before = facts(&database);
        let error = armed(point, || migrate(&database, &directory.join(BACKUP))).unwrap_err().to_string();
        assert!(error.contains("injected failure"), "{point}: {error}");
        assert_eq!(facts(&database), before, "{point}");
        assert_eq!(facts(&directory.join(BACKUP)), before, "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

// ---- Provider import -------------------------------------------------------------

fn whoop_export() -> PathBuf {
    repository().join("tests/fixtures/exports/whoop")
}

fn import_whoop(database: &Path) -> Result<ProviderImportResult> {
    let mut store = Store::open(database, OpenOptions::default())?;
    let options = ImportOptions { received_at: Some(RECEIVED_AT.into()) };
    let preview = import::preview(&store, &whoop_export(), &options)?;
    preview.apply(&mut store)
}

#[test]
#[ignore = "run by run_child"]
fn child_provider_import() {
    if let Some(directory) = child_directory() {
        import_whoop(&directory.join(DATABASE)).unwrap();
    }
}

fn empty_database(directory: &Path) -> PathBuf {
    let path = directory.join(DATABASE);
    Store::create(&path, CreateOptions::default()).unwrap().close().unwrap();
    path
}

/// The import working folders in a temporary folder.
fn working_folders(temporary: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = fs::read_dir(temporary)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("ath-import-"))
        .map(|entry| entry.path())
        .collect();
    folders.sort();
    folders
}

/// True when a new preview finds every record of the export already stored.
fn whoop_fully_imported(database: &Path) -> bool {
    let store = Store::open(database, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let options = ImportOptions { received_at: Some(RECEIVED_AT.into()) };
    let preview = import::preview(&store, &whoop_export(), &options).unwrap();
    preview.new_record_count() == 0 && preview.changed_record_count() == 0 && preview.matching_record_count() > 0
}

#[test]
fn a_stopped_provider_import_writes_all_or_nothing_and_a_later_import_removes_its_plan() {
    for (point, committed) in [("provider.before_commit", false), ("provider.after_commit", true)] {
        let directory = temporary("ath-stop-import-");
        let database = empty_database(&directory);
        let temporary_folder = directory.join("temporary");
        files::create_private_dir(&temporary_folder).unwrap();
        run_child("stopped_writes::child_provider_import", Some(point), &directory, Some(&temporary_folder));

        drop(Store::open(&database, OpenOptions::default()).unwrap());
        let after = facts(&database);
        assert_eq!(after.integrity, "ok", "{point}");
        if committed {
            assert_eq!(after.database_revision, 1, "{point}");
            assert!(whoop_fully_imported(&database), "{point}: the whole import must be stored");
        } else {
            assert_eq!((after.database_revision, after.records), (0, 0), "{point}: nothing may be stored");
        }

        // The stopped process could not remove its staged plan.
        let left = working_folders(&temporary_folder);
        assert_eq!(left.len(), 1, "{point}");
        assert!(left[0].join("plan.sqlite").is_file(), "{point}");

        // A later import in a new process removes it and completes the import.
        run_child("stopped_writes::child_provider_import", None, &directory, Some(&temporary_folder));
        assert!(working_folders(&temporary_folder).is_empty(), "{point}");
        assert!(whoop_fully_imported(&database), "{point}");
        assert_eq!(facts(&database).database_revision, 1, "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_failed_provider_import_writes_nothing() {
    let directory = temporary("ath-fail-import-");
    let database = empty_database(&directory);
    let error = armed("provider.before_commit", || import_whoop(&database)).unwrap_err().to_string();
    assert!(error.contains("injected failure"), "{error}");
    let after = facts(&database);
    assert_eq!((after.database_revision, after.records), (0, 0));
    fs::remove_dir_all(directory).unwrap();
}

// ---- Confirmed repair -------------------------------------------------------------

/// The npm 1.0.0 database: 110 records whose typed rows hold their JSON in
/// another key order.
const REPAIR_FIXTURE: &str = "tests/fixtures/npm-100-encoding/athlete.ath.sqlite";
const DAMAGED_RECORDS: usize = 110;

fn repair(database: &Path, backup: &Path) -> Result<ConfirmedRepairResult> {
    let mut store = Store::open(database, OpenOptions { read_only: false, allow_outdated_schema: true })?;
    execute_confirmed_repair(
        &mut store,
        &RepairOptions { backup_path: Some(backup.to_path_buf()), ..Default::default() },
    )
}

fn repair_candidates(database: &Path) -> usize {
    repair_preview(&open_outdated(database)).unwrap().repair_candidates
}

#[test]
#[ignore = "run by run_child"]
fn child_repair() {
    if let Some(directory) = child_directory() {
        repair(&directory.join(DATABASE), &directory.join(BACKUP)).unwrap();
    }
}

#[test]
fn a_stopped_repair_repairs_nothing_and_can_run_again() {
    for point in ["repair.after_backup", "repair.before_commit"] {
        let directory = temporary("ath-stop-repair-");
        let database = copy_fixture(REPAIR_FIXTURE, &directory);
        let before = facts(&database);
        assert_eq!(repair_candidates(&database), DAMAGED_RECORDS);
        run_child("stopped_writes::child_repair", Some(point), &directory, None);

        assert_eq!(repair_candidates(&database), DAMAGED_RECORDS, "{point}: no row may be rebuilt");
        assert_eq!(facts(&database), before, "{point}");
        assert_eq!(facts(&directory.join(BACKUP)), before, "{point}: the backup is the starting database");

        let result = repair(&database, &directory.join("second-backup.sqlite")).unwrap();
        assert_eq!(result.repaired_record_count, DAMAGED_RECORDS, "{point}");
        assert_eq!(repair_candidates(&database), 0, "{point}");
        assert_eq!(facts(&database).database_revision, before.database_revision + 1, "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_failed_repair_repairs_nothing() {
    for point in ["repair.after_backup", "repair.before_commit"] {
        let directory = temporary("ath-fail-repair-");
        let database = copy_fixture(REPAIR_FIXTURE, &directory);
        let before = facts(&database);
        assert!(armed(point, || repair(&database, &directory.join(BACKUP))).is_err(), "{point}");
        assert_eq!(repair_candidates(&database), DAMAGED_RECORDS, "{point}");
        assert_eq!(facts(&database), before, "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}
