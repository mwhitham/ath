//! The read policy: read-only opens change no file state, read committed WAL
//! data while a writer holds the file, never checkpoint or delete sidecars,
//! and concurrent writers wait for each other instead of failing.
#![cfg_attr(not(unix), allow(unused))]

mod maintenance_support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use athleticstandard::model::*;
use athleticstandard::store::check::check_database;
use athleticstandard::store::migrate::inspect_migration;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::repair::repair_preview;
use athleticstandard::store::stats::database_stats;
use athleticstandard::store::{Changes, CreateOptions, NewRecord, OpenOptions, Store};
use maintenance_support::*;
use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags};

fn report(note: &str) -> NewRecord {
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &serde_json::json!({
            "report_type": "soreness",
            "reported_at": "2026-08-09T12:00:00.000Z",
            "reported_via": "text",
            "note": note,
            "input_reported_at": {
                "timestamp_text": "2026-08-09T07:00:00-05:00",
                "utc_offset_text": "-05:00",
                "utc_offset_meaning": "provider_cycle_offset",
                "timestamp_basis": "source_offset_timestamp"
            }
        }),
    )
    .unwrap();
    NewRecord::new(data, RecordActor::Tool { tool: "ath".into(), tool_version: "2.1.1".into() })
}

fn read_only() -> OpenOptions {
    OpenOptions { read_only: true, allow_outdated_schema: true }
}

/// Every read the maintenance commands make.
fn read_everything(path: &Path) {
    let store = Store::open(path, read_only()).unwrap();
    check_database(&store).unwrap();
    database_stats(&store).unwrap();
    repair_preview(&store).unwrap();
    store.list_records(&ListOptions { include_deleted: true, ..ListOptions::default() }).unwrap();
    drop(store);
    inspect_migration(path, None).unwrap();
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    athleticstandard::files::sidecar(path, suffix)
}

fn optional_hash(path: &Path) -> Option<String> {
    path.exists().then(|| file_hash(path))
}

#[test]
fn reads_leave_a_rollback_journal_database_byte_for_byte() {
    let path = copy_fixture(&demo_fixture(), "ath-read-delete-", true);
    direct(&path).execute_batch("PRAGMA journal_mode = DELETE").unwrap();
    let before = file_hash(&path);
    read_everything(&path);
    assert_eq!(file_hash(&path), before);
    assert_eq!(journal_mode(&path), "delete");
    assert!(!sidecar(&path, "-wal").exists());
    assert!(!sidecar(&path, "-journal").exists());
}

#[test]
fn reads_leave_unhealthy_and_read_only_files_unchanged() {
    // A damaged typed row and an owner-read-only file.
    let path = copy_fixture(&demo_fixture(), "ath-read-unhealthy-", true);
    direct(&path)
        .execute_batch(
            "PRAGMA journal_mode = DELETE;
             UPDATE point_measurements SET standard_value = standard_value + 1;",
        )
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    }
    let before = file_hash(&path);
    read_everything(&path);
    assert_eq!(file_hash(&path), before);
    let store = Store::open(&path, read_only()).unwrap();
    assert!(!check_database(&store).unwrap().valid);
}

#[test]
fn a_mismatched_header_is_refused_without_change() {
    let path = copy_fixture(&demo_fixture(), "ath-read-mismatch-", false);
    direct(&path).execute_batch("PRAGMA journal_mode = DELETE; PRAGMA user_version = 9;").unwrap();
    let before = file_hash(&path);
    let error = Store::open(&path, read_only()).unwrap_err().to_string();
    assert_eq!(error, "SQLite user_version 9 does not match database metadata 4");
    assert_eq!(file_hash(&path), before);
    assert_eq!(journal_mode(&path), "delete");
}

/// A writer that leaves committed frames in the WAL when it closes.
fn wal_with_committed_frames(prefix: &str) -> PathBuf {
    let dir = temporary_dir(prefix);
    let path = dir.join("athlete.ath.sqlite");
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    store.apply(Changes { creates: vec![report("checkpointed")], ..Changes::default() }).unwrap();
    store.close().unwrap();
    let connection = direct(&path);
    connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).unwrap();
    connection.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    drop(connection);
    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    store.connection().set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true).unwrap();
    store.connection().pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    store.apply(Changes { creates: vec![report("only in the WAL")], ..Changes::default() }).unwrap();
    drop(store);
    assert!(std::fs::metadata(sidecar(&path, "-wal")).unwrap().len() > 0);
    path
}

#[test]
fn a_reader_sees_committed_wal_data_and_never_checkpoints_or_deletes_sidecars() {
    let path = wal_with_committed_frames("ath-read-wal-");
    let main_before = file_hash(&path);
    let wal_before = file_hash(&sidecar(&path, "-wal"));
    let wal_identity = athleticstandard::files::identity(&sidecar(&path, "-wal")).unwrap();
    let shm_identity = athleticstandard::files::identity(&sidecar(&path, "-shm")).unwrap();

    let store = Store::open(&path, read_only()).unwrap();
    let records = store.list_records(&ListOptions::default()).unwrap();
    assert_eq!(records.len(), 2, "the reader must see the commit that is only in the WAL");
    assert!(check_database(&store).unwrap().valid);
    drop(store);
    read_everything(&path);

    assert_eq!(file_hash(&path), main_before, "a read checkpointed into the database file");
    assert_eq!(file_hash(&sidecar(&path, "-wal")), wal_before, "a read changed the WAL");
    assert_eq!(athleticstandard::files::identity(&sidecar(&path, "-wal")).unwrap(), wal_identity);
    assert_eq!(athleticstandard::files::identity(&sidecar(&path, "-shm")).unwrap(), shm_identity);
    assert_eq!(journal_mode(&path), "wal");
}

#[test]
fn a_reader_sees_only_committed_data_while_a_writer_holds_the_file() {
    let dir = temporary_dir("ath-read-writer-");
    let path = dir.join("athlete.ath.sqlite");
    let mut writer = Store::create(&path, CreateOptions::default()).unwrap();
    writer.apply(Changes { creates: vec![report("committed")], ..Changes::default() }).unwrap();

    // A second writer holds an open write transaction with uncommitted rows.
    let holder = direct(&path);
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    holder.execute("UPDATE database_state SET current_revision = current_revision + 100", []).unwrap();

    let reader = Store::open(&path, read_only()).unwrap();
    assert_eq!(reader.list_records(&ListOptions::default()).unwrap().len(), 1);
    assert_eq!(reader.database_revision().unwrap(), 1);
    assert!(check_database(&reader).unwrap().valid);
    drop(reader);

    holder.execute_batch("ROLLBACK").unwrap();
    drop(holder);
    writer.close().unwrap();
}

#[test]
fn two_writers_wait_for_each_other() {
    let dir = temporary_dir("ath-read-two-writers-");
    let path = dir.join("athlete.ath.sqlite");
    Store::create(&path, CreateOptions::default()).unwrap().close().unwrap();

    let threads: Vec<_> = (0..2)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&path, OpenOptions::default()).unwrap();
                for index in 0..25 {
                    store
                        .apply(Changes {
                            creates: vec![report(&format!("writer {writer} note {index}"))],
                            ..Changes::default()
                        })
                        .unwrap();
                }
                store.close().unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let store = Store::open(&path, read_only()).unwrap();
    assert_eq!(store.list_records(&ListOptions::default()).unwrap().len(), 50);
    assert_eq!(store.database_revision().unwrap(), 50);
}

#[test]
fn a_writer_waits_for_a_held_lock_within_the_busy_timeout() {
    let dir = temporary_dir("ath-read-busy-");
    let path = dir.join("athlete.ath.sqlite");
    Store::create(&path, CreateOptions::default()).unwrap().close().unwrap();
    let holder = direct(&path);
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        holder.execute_batch("COMMIT").unwrap();
    });
    let started = Instant::now();
    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    store.apply(Changes { creates: vec![report("after the wait")], ..Changes::default() }).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(300), "the write did not wait");
    release.join().unwrap();
    store.close().unwrap();
}

#[test]
fn a_failed_write_keeps_the_sidecars() {
    let dir = temporary_dir("ath-read-failed-write-");
    let path = dir.join("athlete.ath.sqlite");
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    store.apply(Changes { creates: vec![report("first")], ..Changes::default() }).unwrap();
    let wal = athleticstandard::files::identity(&sidecar(&path, "-wal")).unwrap();
    let shm = athleticstandard::files::identity(&sidecar(&path, "-shm")).unwrap();
    let error = store
        .apply(Changes { expected_database_revision: Some(99), creates: vec![report("stale")], ..Changes::default() })
        .unwrap_err();
    assert!(matches!(error, athleticstandard::Error::Stale(_)));
    assert_eq!(athleticstandard::files::identity(&sidecar(&path, "-wal")).unwrap(), wal);
    assert_eq!(athleticstandard::files::identity(&sidecar(&path, "-shm")).unwrap(), shm);
    assert_eq!(store.list_records(&ListOptions::default()).unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn a_wal_database_in_a_read_only_folder_is_refused_or_read_without_change() {
    use std::os::unix::fs::PermissionsExt;
    let path = wal_with_committed_frames("ath-read-locked-folder-");
    let dir = path.parent().unwrap().to_path_buf();
    // Without sidecars, SQLite needs to create them to read a WAL database.
    let checkpoint = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    checkpoint.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    drop(checkpoint);
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(sidecar(&path, suffix));
    }
    let before = file_hash(&path);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result =
        Store::open(&path, read_only()).map(|store| store.list_records(&ListOptions::default()).map(|r| r.len()));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    match result {
        Ok(Ok(count)) => assert_eq!(count, 2),
        Ok(Err(error)) | Err(error) => {
            let message = error.to_string();
            assert_eq!(
                message,
                format!(
                    "Cannot read {} read-only. A WAL database needs its -wal and -shm files, and this folder does not allow creating them. Nothing was changed.",
                    path.display()
                )
            );
        }
    }
    assert_eq!(file_hash(&path), before);
    assert_eq!(optional_hash(&sidecar(&path, "-wal")), None);
}
