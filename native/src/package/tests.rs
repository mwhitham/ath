//! Failure injection at every file and SQL boundary of the package operations.
//!
//! Each boundary is tested two ways. An armed point makes the operation fail
//! there and run its own error path; afterwards no athlete data changed and no
//! file or pending entry is left. A child test process stopped at the point
//! runs no cleanup at all, as a killed process would; the parent then reopens
//! the database and checks that the data is usable, that no record names a
//! missing file, and that the pending cleanup finishes the file work.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use super::failpoints::{self, STOP_STATUS, STOP_VARIABLE};
use super::*;
use crate::error::Result;
use crate::files;
use crate::model::*;
use crate::store::{self, read, Changes, CreateOptions, NewRecord, OpenOptions, Store};

const DIRECTORY_VARIABLE: &str = "ATH_TEST_DIRECTORY";
const DEMO_ATTACHMENT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000e.png";
const ATTACHMENT_ID: &str = "01890f3e-e7d3-7cc0-88c8-00000000000e";

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn temporary(prefix: &str) -> PathBuf {
    files::make_temporary_directory(prefix).unwrap()
}

fn open(path: &Path) -> Store {
    Store::open(path, OpenOptions::default()).unwrap()
}

fn demo_attachment_source() -> PathBuf {
    repository().join("examples/demo-athlete/attachments").join(DEMO_ATTACHMENT)
}

/// The demo database and attachment, migrated to the current schema.
fn demo_copy(directory: &Path) -> PathBuf {
    let path = directory.join("athlete.ath.sqlite");
    fs::copy(repository().join("examples/demo-athlete/athlete.ath.sqlite"), &path).unwrap();
    files::set_owner_only(&path).unwrap();
    files::create_private_dir(&directory.join("attachments")).unwrap();
    fs::copy(demo_attachment_source(), directory.join("attachments").join(DEMO_ATTACHMENT)).unwrap();
    let mut opened = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    opened.transaction(|tx| store::migrate::apply_step(tx, 5).map(|_| ())).unwrap();
    opened.close().unwrap();
    path
}

fn empty_database(directory: &Path) -> PathBuf {
    let path = directory.join("destination.ath.sqlite");
    Store::create(&path, CreateOptions::default()).unwrap().close().unwrap();
    path
}

/// The 2.1.0 demo package from the golden files, as a package folder.
fn demo_package(directory: &Path) -> PathBuf {
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/package/demo");
    let target = directory.join("package-in");
    fs::create_dir_all(target.join("series")).unwrap();
    fs::create_dir_all(target.join("attachments")).unwrap();
    fs::copy(golden.join("main.json"), target.join(PACKAGE_DOCUMENT)).unwrap();
    fs::copy(golden.join("manifest.json"), target.join(MANIFEST_DOCUMENT)).unwrap();
    for entry in fs::read_dir(golden.join("series")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), target.join("series").join(entry.file_name())).unwrap();
    }
    fs::copy(demo_attachment_source(), target.join("attachments").join(DEMO_ATTACHMENT)).unwrap();
    target
}

fn files_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .map(|entries| entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

fn pending(path: &Path) -> Vec<PendingFileOperation> {
    let store = Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    pending_file_operations(store.connection()).unwrap()
}

/// Every attachment record's file exists with its recorded size.
fn assert_no_missing_attachment_files(path: &Path) {
    let store = Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let records = store
        .list_records(&read::ListOptions {
            record_type: Some(RecordType::Attachment),
            include_deleted: true,
            limit: 10_000,
        })
        .unwrap();
    for record in records {
        let RecordData::Attachment(data) = &record.data else { unreachable!() };
        let file = attachment_directory(path).join(stored_attachment_name(&record.record_id, data));
        let metadata =
            fs::metadata(&file).unwrap_or_else(|_| panic!("record names a missing file: {}", file.display()));
        assert_eq!(metadata.len(), data.byte_count);
    }
}

/// Run one ignored child test in a new process that stops at `point`.
fn stop_child_at(test: &str, point: &str, directory: &Path) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(STOP_VARIABLE, point)
        .env(DIRECTORY_VARIABLE, directory)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(STOP_STATUS), "{test} did not stop at {point}");
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

// ---- Attachment writes --------------------------------------------------------------

const ATTACHMENT_POINTS: &[&str] = &[
    "attachments.after_pending_rows",
    "attachments.after_temporary_write",
    "attachments.after_rename",
    "attachments.after_directory_sync",
    "attachments.before_commit",
];

fn report_with(attachment_id: &str) -> NewRecord {
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &json!({
            "report_type": "note",
            "reported_at": "2026-11-14T08:00:00.000Z",
            "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
            "reported_via": "photo",
            "attachment_ids": [attachment_id],
        }),
    )
    .unwrap();
    NewRecord::new(data, RecordActor::Athlete)
}

fn write_attachment(database: &Path) -> Result<store::Applied> {
    let mut store = open(database);
    let prepared =
        prepare_attachments(&store, &[demo_attachment_source()], &RecordActor::Athlete, "2026-11-14T08:00:00.000Z")?;
    let changes =
        Changes { creates: vec![prepared[0].new_record(), report_with(&prepared[0].record_id)], ..Changes::default() };
    apply_with_attachments(&mut store, &prepared, changes)
}

#[test]
#[ignore = "run by stop_child_at"]
fn child_attachment_write() {
    if let Some(directory) = child_directory() {
        write_attachment(&directory.join("destination.ath.sqlite")).unwrap();
    }
}

#[test]
fn a_failed_attachment_write_leaves_nothing_behind() {
    for point in ATTACHMENT_POINTS {
        let directory = temporary("ath-fail-attach-");
        let database = empty_database(&directory);
        assert!(armed(point, || write_attachment(&database)).is_err(), "{point}");
        assert!(files_in(&directory.join("attachments")).is_empty(), "{point}");
        assert!(pending(&database).is_empty(), "{point}");
        assert_eq!(open(&database).database_revision().unwrap(), 0, "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_stopped_attachment_write_leaves_only_unreferenced_files_that_cleanup_removes() {
    for point in ATTACHMENT_POINTS {
        let directory = temporary("ath-stop-attach-");
        let database = empty_database(&directory);
        stop_child_at("package::tests::child_attachment_write", point, &directory);
        let mut store = open(&database);
        assert_eq!(store.database_revision().unwrap(), 0, "{point}: no record may commit");
        assert!(!pending(&database).is_empty(), "{point}: the files stay owned by a pending entry");
        let report = resume_pending_file_operations(&mut store).unwrap();
        assert!(report.is_complete(), "{point}: {report:?}");
        assert!(files_in(&directory.join("attachments")).is_empty(), "{point}");
        assert!(pending(&database).is_empty(), "{point}");
        // The database is usable: the same write now succeeds.
        drop(store);
        write_attachment(&database).unwrap();
        assert_no_missing_attachment_files(&database);
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn an_attachment_write_stopped_after_commit_is_complete() {
    let directory = temporary("ath-stop-attach-commit-");
    let database = empty_database(&directory);
    stop_child_at("package::tests::child_attachment_write", "attachments.after_commit", &directory);
    assert_eq!(open(&database).database_revision().unwrap(), 1);
    assert!(pending(&database).is_empty());
    assert_eq!(files_in(&directory.join("attachments")).len(), 1);
    assert_no_missing_attachment_files(&database);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cleanup_leaves_a_running_write_alone() {
    let directory = temporary("ath-attach-concurrent-");
    let database = empty_database(&directory);
    let cleaner = database.clone();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut store = open(&cleaner);
        let mut active_seen = false;
        while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let report = resume_pending_file_operations(&mut store).unwrap();
            assert!(report.removed_files.is_empty(), "{report:?}");
            active_seen |= !report.active_operations.is_empty();
        }
        active_seen
    });
    // Pause the write while its files exist and are still unrecorded.
    failpoints::on("attachments.after_directory_sync", || std::thread::sleep(std::time::Duration::from_millis(200)));
    let result = write_attachment(&database);
    failpoints::disarm_all();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let active_seen = handle.join().unwrap();
    result.unwrap();
    assert!(active_seen, "the cleaner should have seen the running write");
    assert_no_missing_attachment_files(&database);
    fs::remove_dir_all(directory).unwrap();
}

// ---- Erasure ---------------------------------------------------------------------------

fn erase_attachment(database: &Path) -> Result<ErasureResult> {
    let mut store = open(database);
    let preview = preview_erasure(&store, ATTACHMENT_ID, 1)?;
    erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, None)
}

#[test]
#[ignore = "run by stop_child_at"]
fn child_erase() {
    if let Some(directory) = child_directory() {
        erase_attachment(&directory.join("athlete.ath.sqlite")).unwrap();
    }
}

fn erased(database: &Path) -> bool {
    open(database).get_record(ATTACHMENT_ID).unwrap().is_none()
}

#[test]
fn an_erasure_that_fails_before_commit_changes_nothing() {
    let directory = temporary("ath-fail-erase-");
    let database = demo_copy(&directory);
    assert!(armed("erase.before_commit", || erase_attachment(&database)).is_err());
    assert!(!erased(&database));
    assert!(directory.join("attachments").join(DEMO_ATTACHMENT).exists());
    assert!(pending(&database).is_empty());
    assert_eq!(open(&database).database_revision().unwrap(), 3);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_erasure_whose_file_step_fails_is_not_reported_as_complete() {
    for point in ["erase.remove_file", "erase.after_file_removal"] {
        let directory = temporary("ath-fail-erase-file-");
        let database = demo_copy(&directory);
        let error = armed(point, || erase_attachment(&database)).unwrap_err();
        assert!(erased(&database), "{point}: the records stay erased");
        assert_eq!(pending(&database).len(), 1, "{point}: {error}");
        let mut store = open(&database);
        let report = resume_pending_file_operations(&mut store).unwrap();
        assert!(report.is_complete(), "{point}");
        assert!(!directory.join("attachments").join(DEMO_ATTACHMENT).exists(), "{point}");
        assert!(pending(&database).is_empty(), "{point}");
        // Windows cannot remove a folder that holds an open database.
        drop(store);
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_stopped_erasure_is_finished_by_a_later_writable_operation() {
    for (point, committed) in [
        ("erase.before_commit", false),
        ("erase.after_commit", true),
        ("erase.remove_file", true),
        ("erase.after_file_removal", true),
    ] {
        let directory = temporary("ath-stop-erase-");
        let database = demo_copy(&directory);
        stop_child_at("package::tests::child_erase", point, &directory);
        assert_eq!(erased(&database), committed, "{point}");
        let file = directory.join("attachments").join(DEMO_ATTACHMENT);
        if committed {
            assert_eq!(pending(&database).len(), 1, "{point}: the removal stays visible");
            let mut store = open(&database);
            assert!(resume_pending_file_operations(&mut store).unwrap().is_complete(), "{point}");
            assert!(!file.exists(), "{point}");
        } else {
            assert!(file.exists(), "{point}");
            assert_eq!(open(&database).database_revision().unwrap(), 3, "{point}");
            assert_no_missing_attachment_files(&database);
        }
        assert!(pending(&database).is_empty(), "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

// ---- Export ------------------------------------------------------------------------------

const EXPORT_POINTS: &[&str] = &["export.after_series", "export.before_manifest", "export.before_rename"];

fn export_demo(database: &Path, output: &Path) -> Result<ExportResult> {
    let store = Store::open(database, OpenOptions { read_only: true, allow_outdated_schema: false })?;
    export_package(&store, output, &ExportOptions::default())
}

#[test]
#[ignore = "run by stop_child_at"]
fn child_export() {
    if let Some(directory) = child_directory() {
        export_demo(&directory.join("athlete.ath.sqlite"), &directory.join("package")).unwrap();
    }
}

fn staging_directories(directory: &Path) -> Vec<String> {
    files_in(directory).into_iter().filter(|name| name.starts_with(files::staging::EXPORT_STAGING_PREFIX)).collect()
}

#[test]
fn a_failed_export_publishes_nothing_and_removes_its_staging() {
    for point in EXPORT_POINTS {
        let directory = temporary("ath-fail-export-");
        let database = demo_copy(&directory);
        let output = directory.join("package");
        assert!(armed(point, || export_demo(&database, &output)).is_err(), "{point}");
        assert!(!output.exists(), "{point}");
        assert!(staging_directories(&directory).is_empty(), "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_stopped_export_never_appears_partial_and_the_next_export_cleans_up() {
    for point in EXPORT_POINTS {
        let directory = temporary("ath-stop-export-");
        let database = demo_copy(&directory);
        stop_child_at("package::tests::child_export", point, &directory);
        assert!(!directory.join("package").exists(), "{point}");
        assert_eq!(staging_directories(&directory).len(), 1, "{point}");
        export_demo(&database, &directory.join("package-two")).unwrap();
        assert!(staging_directories(&directory).is_empty(), "{point}");
        assert!(load_package(&directory.join("package-two")).is_ok(), "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn an_export_stopped_after_publication_is_complete() {
    let directory = temporary("ath-stop-export-after-");
    let database = demo_copy(&directory);
    stop_child_at("package::tests::child_export", "export.after_rename", &directory);
    let output = directory.join("package");
    assert!(output.join(MANIFEST_DOCUMENT).is_file());
    assert!(load_package(&output).is_ok());
    // A later export leaves the published package in place.
    export_demo(&database, &directory.join("package-two")).unwrap();
    assert!(output.join(MANIFEST_DOCUMENT).is_file());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_export_reads_one_snapshot_while_another_writer_commits() {
    let directory = temporary("ath-export-snapshot-");
    let database = demo_copy(&directory);
    let writer = database.clone();
    failpoints::on("export.after_series", move || {
        let mut store = open(&writer);
        let data = RecordData::decode(
            RecordType::AthleteReport,
            &json!({
                "report_type": "note",
                "reported_at": "2026-11-14T09:00:00.000Z",
                "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
                "reported_via": "text",
                "note": "Written during the export",
            }),
        )
        .unwrap();
        store
            .apply(Changes { creates: vec![NewRecord::new(data, RecordActor::Athlete)], ..Changes::default() })
            .unwrap();
        store.close().unwrap();
    });
    let result = export_demo(&database, &directory.join("package"));
    failpoints::disarm_all();
    let result = result.unwrap();
    assert_eq!(result.database_revision, 3);
    let loaded = load_package(&directory.join("package")).unwrap();
    assert_eq!(loaded.database_revision, 3);
    assert_eq!(loaded.records.len(), 24);
    let reports = result.manifest.record_counts.iter().find(|count| count.record_type == RecordType::AthleteReport);
    assert_eq!(reports.unwrap().current_record_count, 1);
    assert_eq!(open(&database).database_revision().unwrap(), 4);
    fs::remove_dir_all(directory).unwrap();
}

// ---- Restore ------------------------------------------------------------------------------

const RESTORE_POINTS: &[&str] = &[
    "attachments.after_pending_rows",
    "attachments.after_rename",
    "attachments.after_directory_sync",
    "restore.after_series_file",
    "restore.before_commit",
];

fn restore_demo(directory: &Path) -> Result<PackageImportApplied> {
    let mut store = open(&directory.join("destination.ath.sqlite"));
    let preview = preview_package_import(&store, &directory.join("package-in"))?;
    apply_package_import(&mut store, &preview, &[])
}

#[test]
#[ignore = "run by stop_child_at"]
fn child_restore() {
    if let Some(directory) = child_directory() {
        restore_demo(&directory).unwrap();
    }
}

fn assert_empty(database: &Path, point: &str) {
    let store = open(database);
    assert_eq!(store.database_revision().unwrap(), 0, "{point}");
    let records = store.list_records(&read::ListOptions { include_deleted: true, ..Default::default() }).unwrap();
    assert!(records.is_empty(), "{point}");
}

/// A restore into an empty database runs in a rollback journal. Afterwards
/// the file is in WAL mode, as every database is, and no journal is left.
fn assert_wal_without_journal(database: &Path, point: &str) {
    let mode: String = rusqlite::Connection::open(database)
        .unwrap()
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal", "{point}");
    assert!(!files::sidecar(database, "-journal").exists(), "{point}");
}

#[test]
fn a_failed_restore_writes_nothing() {
    for point in RESTORE_POINTS {
        let directory = temporary("ath-fail-restore-");
        let database = empty_database(&directory);
        demo_package(&directory);
        assert!(armed(point, || restore_demo(&directory)).is_err(), "{point}");
        assert_wal_without_journal(&database, point);
        assert_empty(&database, point);
        assert!(files_in(&directory.join("attachments")).is_empty(), "{point}");
        assert!(pending(&database).is_empty(), "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_stopped_restore_is_not_partial_and_can_run_again() {
    for point in RESTORE_POINTS {
        let directory = temporary("ath-stop-restore-");
        let database = empty_database(&directory);
        demo_package(&directory);
        stop_child_at("package::tests::child_restore", point, &directory);
        assert_empty(&database, point);
        let mut store = open(&database);
        assert!(resume_pending_file_operations(&mut store).unwrap().is_complete(), "{point}");
        assert!(files_in(&directory.join("attachments")).is_empty(), "{point}");
        drop(store);
        let applied = restore_demo(&directory).unwrap();
        assert!(applied.restored, "{point}");
        assert_wal_without_journal(&database, point);
        assert_no_missing_attachment_files(&database);
        assert!(pending(&database).is_empty(), "{point}");
        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn a_restore_stopped_after_commit_is_complete() {
    let directory = temporary("ath-stop-restore-after-");
    let database = empty_database(&directory);
    demo_package(&directory);
    stop_child_at("package::tests::child_restore", "restore.after_commit", &directory);
    assert_wal_without_journal(&database, "restore.after_commit");
    assert_eq!(open(&database).database_revision().unwrap(), 3);
    assert!(pending(&database).is_empty());
    assert_no_missing_attachment_files(&database);
    fs::remove_dir_all(directory).unwrap();
}

/// A rollback-journal write that has written pages to the file and then
/// stops, as a killed restore into a new database does.
#[test]
#[ignore = "run by stop_child_at"]
fn child_rollback_journal_write() {
    if let Some(directory) = child_directory() {
        let connection = rusqlite::Connection::open(directory.join("destination.ath.sqlite")).unwrap();
        let mode: String =
            connection.pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0)).unwrap();
        assert_eq!(mode, "delete");
        // A small page cache makes SQLite write pages to the file before commit.
        connection.pragma_update(None, "cache_size", 8).unwrap();
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 UPDATE database_state SET current_revision = 9 WHERE singleton_id = 1;
                 CREATE TABLE filler(bytes BLOB);",
            )
            .unwrap();
        for _ in 0..64 {
            connection.execute("INSERT INTO filler VALUES (zeroblob(8192))", []).unwrap();
        }
        std::process::exit(STOP_STATUS);
    }
}

/// After a killed restore into a new database, the file reads as the empty
/// database it was, through a read-only open as well, and is back in WAL
/// mode with no journal left.
#[test]
fn a_stopped_rollback_journal_write_reads_as_before() {
    let directory = temporary("ath-stopped-write-");
    let database = empty_database(&directory);
    stop_child_at("package::tests::child_rollback_journal_write", "rollback journal write", &directory);
    assert!(files::sidecar(&database, "-journal").exists());
    let store = Store::open(&database, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    assert_eq!(store.database_revision().unwrap(), 0);
    drop(store);
    assert_wal_without_journal(&database, "stopped write");
    assert_empty(&database, "stopped write");
    let filler: i64 = rusqlite::Connection::open(&database)
        .unwrap()
        .query_row("SELECT count(*) FROM sqlite_schema WHERE name = 'filler'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(filler, 0);
    fs::remove_dir_all(directory).unwrap();
}

// ---- Decoding checked package items ------------------------------------------

/// Loading checks each package file with its schema, then decodes the items
/// without checking each one again. The result must equal a full check and
/// decode of every item.
#[test]
fn package_items_decode_as_a_full_check_would() {
    let directory = temporary("ath-package-decode-");
    let package = demo_package(&directory);
    let loaded = load_package(&package).unwrap();
    let document = crate::json::parse(&fs::read_to_string(package.join(PACKAGE_DOCUMENT)).unwrap()).unwrap();
    let items = |value: &crate::json::Value, key: &str| value[key].as_array().unwrap().clone();
    let records: Vec<CurrentRecord> =
        items(&document, "records").iter().map(|item| CurrentRecord::from_value_non_sample(item).unwrap()).collect();
    let revisions: Vec<RecordRevision> = items(&document, "record_revisions")
        .iter()
        .map(|item| RecordRevision::from_value_non_sample(item).unwrap())
        .collect();
    assert!(!records.is_empty() && !revisions.is_empty());
    assert_eq!(loaded.records, records);
    assert_eq!(loaded.revisions, revisions);

    assert!(!loaded.series_files.is_empty());
    for file in &loaded.series_files {
        let batch = load::read_series_batch(file).unwrap();
        let value = crate::json::parse(&fs::read_to_string(&file.path).unwrap()).unwrap();
        let records: Vec<CurrentRecord> =
            items(&value, "records").iter().map(|item| CurrentRecord::from_value(item).unwrap()).collect();
        let revisions: Vec<RecordRevision> =
            items(&value, "record_revisions").iter().map(|item| RecordRevision::from_value(item).unwrap()).collect();
        assert!(!records.is_empty() && !revisions.is_empty());
        assert_eq!(batch.records, records);
        assert_eq!(batch.revisions, revisions);
    }
    fs::remove_dir_all(directory).unwrap();
}

/// A series file changed after load is refused before any item is decoded,
/// and loading the changed package again is refused too.
#[test]
fn a_series_file_changed_after_load_is_refused() {
    let directory = temporary("ath-package-broken-sample-");
    let package = demo_package(&directory);
    let loaded = load_package(&package).unwrap();
    let file = &loaded.series_files[0];
    let text = fs::read_to_string(&file.path).unwrap();
    let mut value = crate::json::parse(&text).unwrap();
    value["records"][0]["data"]["recorded_at"] = crate::json::Value::String("not a time".into());
    fs::write(&file.path, crate::json::to_deterministic_pretty(&value)).unwrap();
    assert!(load::read_series_batch(file).is_err());
    assert!(matches!(load_package_checked(&package), Err(_) | Ok(PackageLoad::Unrepresentable(_))));
    fs::remove_dir_all(directory).unwrap();
}

// ---- Streamed restore ----------------------------------------------------------

/// The demo package with a second series file that repeats the first file's
/// sample under another source. Loading accepts it: the files belong to
/// different groups. Writing it fails on the repeated record ID.
fn package_with_a_repeated_sample(directory: &Path) -> PathBuf {
    let package = demo_package(directory);
    fs::remove_file(package.join(MANIFEST_DOCUMENT)).unwrap();
    let first = fs::read_dir(package.join("series")).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(&first).unwrap();
    let source = "01890f3e-e7d3-7cc0-88c8-000000000003";
    let other = "01890f3e-e7d3-7cc0-88c8-000000000004";
    let copy = text.replace(source, other);
    let name = format!("series/{other}-heart_rate-000001.ath-series.json");
    fs::write(package.join(&name), &copy).unwrap();
    let mut document = crate::json::parse(&fs::read_to_string(package.join(PACKAGE_DOCUMENT)).unwrap()).unwrap();
    let mut reference = document["measurement_series_files"][0].clone();
    reference["file"] = serde_json::Value::String(name);
    reference["source_id"] = serde_json::Value::String(other.into());
    reference["sha256"] = serde_json::Value::String(files::durable::sha256_hex(copy.as_bytes()));
    document["measurement_series_files"].as_array_mut().unwrap().push(reference);
    fs::write(package.join(PACKAGE_DOCUMENT), crate::json::to_deterministic_pretty(&document)).unwrap();
    package
}

/// When a streamed restore fails part way, it is rolled back and run again
/// reading each file whole, so the refusal is the one the whole reading
/// gives: here the repeated record ID on `records`, not on `record_revisions`,
/// which the streamed order would reach first.
#[test]
fn a_failed_streamed_restore_reports_the_whole_reading_error() {
    let directory = temporary("ath-streamed-restore-failure-");
    let database = empty_database(&directory);
    let package = package_with_a_repeated_sample(&directory);
    let mut store = open(&database);
    let preview = preview_package_import(&store, &package).unwrap();
    assert!(preview.restore && preview.conflicts.is_empty());
    let error = apply_package_import(&mut store, &preview, &[]).unwrap_err().to_string();
    assert!(error.contains("UNIQUE constraint failed: records.record_id"), "{error}");
    drop(store);
    assert_wal_without_journal(&database, "streamed failure");
    assert_empty(&database, "streamed failure");
    fs::remove_dir_all(directory).unwrap();
}
