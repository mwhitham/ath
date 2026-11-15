//! Verified backups through SQLite's in-process backup API.
#![cfg_attr(not(unix), allow(unused))]

mod maintenance_support;

use std::path::Path;

use athleticstandard::model::*;
use athleticstandard::store::backup::{create_verified_backup, default_migration_backup_path};
use athleticstandard::store::{read_metadata, Changes, CreateOptions, NewRecord, OpenOptions, Store};
use maintenance_support::*;

fn tool_actor() -> RecordActor {
    RecordActor::Tool { tool: "ath".into(), tool_version: "2.1.1".into() }
}

fn report(text: &str) -> NewRecord {
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &serde_json::json!({
            "report_type": "soreness",
            "reported_at": "2026-08-09T12:00:00.000Z",
            "reported_via": "text",
            "note": text,
            "input_reported_at": {
                "timestamp_text": "2026-08-09T07:00:00-05:00",
                "utc_offset_text": "-05:00",
                "utc_offset_meaning": "provider_cycle_offset",
                "timestamp_basis": "source_offset_timestamp"
            }
        }),
    )
    .unwrap();
    NewRecord::new(data, tool_actor())
}

fn new_database(prefix: &str) -> (std::path::PathBuf, Store) {
    let dir = temporary_dir(prefix);
    let path = dir.join("athlete.ath.sqlite");
    let store = Store::create(&path, CreateOptions::default()).unwrap();
    (path, store)
}

#[test]
fn a_backup_is_one_verified_owner_only_file() {
    let path = copy_fixture(&demo_fixture(), "ath-backup-", false);
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let backup_path = default_migration_backup_path(&path, 0);
    let verified = store.create_verified_backup(&backup_path).unwrap();
    assert_eq!(verified.path, backup_path);
    assert_eq!(verified.database_id, store.metadata().database_id);
    assert_eq!(verified.sql_schema_version, 4);
    assert_eq!(verified.database_revision as i64, database_revision(&path));
    assert_eq!(verified.record_count, 25);
    assert_eq!(file_mode(&backup_path), 0o600);
    assert_eq!(journal_mode(&backup_path), "delete");
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!athleticstandard::files::sidecar(&backup_path, suffix).exists(), "{suffix}");
    }
    assert_eq!(dump_tables(&backup_path, ATHLETE_TABLES), dump_tables(&path, ATHLETE_TABLES));
    let integrity: String = inspect(&backup_path).query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn an_existing_backup_is_never_replaced() {
    let (path, store) = new_database("ath-backup-existing-");
    let backup_path = athleticstandard::files::sidecar(&path, ".backup.sqlite");
    std::fs::write(&backup_path, "stale").unwrap();
    let error = store.create_verified_backup(&backup_path).unwrap_err().to_string();
    assert_eq!(error, format!("Refusing to overwrite existing migration backup: {}", backup_path.display()));
    assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), "stale");
}

#[cfg(unix)]
#[test]
fn a_backup_path_through_a_symbolic_link_is_refused() {
    let (path, store) = new_database("ath-backup-link-");
    let dir = path.parent().unwrap();
    let link = dir.join("backup-link.sqlite");
    std::os::unix::fs::symlink(dir.join("target.sqlite"), &link).unwrap();
    let error = store.create_verified_backup(&link).unwrap_err().to_string();
    assert!(error.contains("symbolic link"), "{error}");
    assert!(!dir.join("target.sqlite").exists());

    let real = dir.join("real");
    std::fs::create_dir(&real).unwrap();
    let linked_dir = dir.join("linked");
    std::os::unix::fs::symlink(&real, &linked_dir).unwrap();
    let error = store.create_verified_backup(&linked_dir.join("backup.sqlite")).unwrap_err().to_string();
    assert!(error.contains("symbolic link"), "{error}");
    assert!(directory_names(&real).is_empty());
}

#[test]
fn a_backup_that_fails_verification_is_removed() {
    let (path, store) = new_database("ath-backup-verify-");
    let backup_path = athleticstandard::files::sidecar(&path, ".backup.sqlite");
    let mut expected = read_metadata(store.connection()).unwrap();
    expected.database_id = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f99".to_string();
    let error = create_verified_backup(store.connection(), &backup_path, &expected).unwrap_err().to_string();
    assert_eq!(error, "Migration backup database_id does not match the source database");
    assert!(!backup_path.exists());
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!athleticstandard::files::sidecar(&backup_path, suffix).exists());
    }

    let mut expected = read_metadata(store.connection()).unwrap();
    expected.sql_schema_version = 4;
    let error = create_verified_backup(store.connection(), &backup_path, &expected).unwrap_err().to_string();
    assert_eq!(error, "Migration backup sql_schema_version does not match the source database");
    assert!(!backup_path.exists());
}

#[test]
fn backups_work_at_unusual_paths() {
    let (path, store) = new_database("ath-backup-names-");
    let dir = path.parent().unwrap();
    // `"` and `\` cannot appear in a Windows file name; `\` separates folders there.
    let names: &[&str] = if cfg!(windows) {
        &["athlete's backup.sqlite", "spaces and ünïcödé 🏃.sqlite"]
    } else {
        &["athlete's backup.sqlite", "a\"b.sqlite", "a\\b.sqlite", "a\\tb.sqlite", "spaces and ünïcödé 🏃.sqlite"]
    };
    for name in names {
        let backup_path = dir.join(name);
        let verified = store.create_verified_backup(&backup_path).unwrap();
        assert!(verified.path.is_file(), "{name}");
        assert_eq!(file_mode(&backup_path), 0o600, "{name}");
    }
}

#[test]
fn a_backup_includes_committed_data_still_in_the_wal() {
    let (path, mut store) = new_database("ath-backup-wal-");
    store.apply(Changes { creates: vec![report("first")], ..Changes::default() }).unwrap();
    store.apply(Changes { creates: vec![report("second")], ..Changes::default() }).unwrap();
    // The writer stays open, so the commits are still in the WAL.
    let wal = athleticstandard::files::sidecar(&path, "-wal");
    assert!(std::fs::metadata(&wal).unwrap().len() > 0);
    let reader = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let backup_path = athleticstandard::files::sidecar(&path, ".backup.sqlite");
    let verified = reader.create_verified_backup(&backup_path).unwrap();
    assert_eq!(verified.record_count, 2);
    assert_eq!(verified.database_revision, 2);
    assert!(std::fs::metadata(&wal).unwrap().len() > 0, "the backup must not checkpoint the live WAL");
    drop(reader);
    drop(store);
    assert_eq!(
        inspect(Path::new(&backup_path))
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
