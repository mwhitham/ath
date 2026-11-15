//! Attachment ingestion for `ath log` and `ath correct`, and cleanup of file
//! changes that an earlier operation left pending.
//! Ported from the attachment cases of tests/v1-cli-writes.test.ts.
#![cfg_attr(not(unix), allow(unused))]

#[path = "package_support/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::model::*;
use athleticstandard::package::*;
use athleticstandard::store::{Changes, NewRecord, Store};
use common::*;
use serde_json::json;

fn new_database(directory: &Path) -> PathBuf {
    empty_destination(directory)
}

fn photo(directory: &Path) -> PathBuf {
    let path = directory.join("meal photo.png");
    fs::copy(repository().join("examples/demo-athlete/attachments").join(DEMO_ATTACHMENT), &path).unwrap();
    path
}

fn report(attachment_ids: &[String]) -> NewRecord {
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &json!({
            "report_type": "note",
            "reported_at": "2026-11-14T08:00:00.000Z",
            "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
            "reported_via": "photo",
            "note": "Meal photo",
            "attachment_ids": attachment_ids,
        }),
    )
    .unwrap();
    NewRecord::new(data, RecordActor::Athlete)
}

fn attachment_files(directory: &Path) -> Vec<String> {
    fs::read_dir(directory.join("attachments"))
        .map(|entries| entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn an_attachment_is_copied_and_linked_in_one_change() {
    let directory = temporary("ath-attach-");
    let database = new_database(&directory);
    let source = photo(&directory);
    let mut store = open(&database);
    let prepared =
        prepare_attachments(&store, std::slice::from_ref(&source), &RecordActor::Athlete, "2026-11-14T08:00:00.000Z")
            .unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].data.file_name, "meal photo.png");
    assert_eq!(prepared[0].data.media_type, "image/png");
    // Preparing writes nothing.
    assert!(attachment_files(&directory).is_empty());
    let ids = vec![prepared[0].record_id.clone()];
    let changes = Changes { creates: vec![prepared[0].new_record(), report(&ids)], ..Changes::default() };
    let applied = apply_with_attachments(&mut store, &prepared, changes).unwrap();
    assert_eq!(applied.database_revision, 1);
    let stored = directory.join("attachments").join(prepared[0].stored_name());
    assert_eq!(prepared[0].final_path, stored);
    assert_eq!(fs::read(&stored).unwrap(), fs::read(&source).unwrap());
    assert_eq!(attachment_files(&directory), [prepared[0].stored_name()]);
    assert!(pending_file_operations(store.connection()).unwrap().is_empty());
    let attachment = store.get_record(&prepared[0].record_id).unwrap().unwrap();
    assert_eq!(attachment.record_type(), RecordType::Attachment);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&stored).unwrap().permissions().mode() & 0o777, 0o600);
    }
    common::remove_dir(&directory);
}

#[test]
fn unsupported_oversized_and_irregular_files_are_refused() {
    let directory = temporary("ath-attach-refused-");
    let database = new_database(&directory);
    let store = open(&database);
    let text = directory.join("notes.txt");
    fs::write(&text, "hello").unwrap();
    let error =
        prepare_attachments(&store, std::slice::from_ref(&text), &RecordActor::Athlete, EXPORTED_AT).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("unsupported attachment type: {}; use PNG, JPEG, WebP, or PDF", text.display())
    );
    let big = directory.join("big.png");
    fs::write(&big, b"\x89PNG\r\n\x1a\n").unwrap();
    fs::OpenOptions::new().write(true).open(&big).unwrap().set_len(25 * 1024 * 1024 + 1).unwrap();
    let error =
        prepare_attachments(&store, std::slice::from_ref(&big), &RecordActor::Athlete, EXPORTED_AT).unwrap_err();
    assert_eq!(error.to_string(), format!("attachment exceeds 26214400 bytes: {}", big.display()));
    let folder = directory.join("folder");
    fs::create_dir(&folder).unwrap();
    let error =
        prepare_attachments(&store, std::slice::from_ref(&folder), &RecordActor::Athlete, EXPORTED_AT).unwrap_err();
    assert_eq!(error.to_string(), format!("attachment is not a regular file: {}", folder.display()));
    common::remove_dir(&directory);
}

#[test]
fn a_failed_change_removes_the_new_files() {
    let directory = temporary("ath-attach-failed-");
    let database = new_database(&directory);
    let source = photo(&directory);
    let mut store = open(&database);
    let prepared = prepare_attachments(&store, &[source], &RecordActor::Athlete, EXPORTED_AT).unwrap();
    let ids = vec![prepared[0].record_id.clone()];
    let stale = Changes {
        expected_database_revision: Some(9),
        creates: vec![prepared[0].new_record(), report(&ids)],
        ..Changes::default()
    };
    assert!(apply_with_attachments(&mut store, &prepared, stale).is_err());
    assert!(attachment_files(&directory).is_empty());
    assert!(pending_file_operations(store.connection()).unwrap().is_empty());
    assert_eq!(store.database_revision().unwrap(), 0);

    // A change that does not create the attachment record is refused first.
    let missing = Changes { creates: vec![report(&[])], ..Changes::default() };
    assert!(apply_with_attachments(&mut store, &prepared, missing).is_err());
    assert!(attachment_files(&directory).is_empty());
    assert_eq!(store.database_revision().unwrap(), 0);
    common::remove_dir(&directory);
}

fn pend(store: &Store, operation_id: &str, kind: &str, relative_path: &str) {
    store
        .connection()
        .execute(
            "INSERT INTO internal_pending_file_operations (operation_id, operation_kind, relative_path, created_at)
             VALUES (?1, ?2, ?3, '2026-11-14T00:00:00.000Z')",
            [operation_id, kind, relative_path],
        )
        .unwrap();
}

#[test]
fn cleanup_removes_only_unreferenced_files_of_stopped_operations() {
    let directory = temporary("ath-attach-resume-");
    let database = migrated_demo_copy(&directory);
    let attachments = directory.join("attachments");
    fs::write(attachments.join("01890f3e-0000-7000-8000-0000000000aa.png"), b"orphan").unwrap();
    fs::write(attachments.join("01890f3e-0000-7000-8000-0000000000bb.png"), b"active").unwrap();
    let mut store = open(&database);
    // A stopped write: one orphan, one temporary name that is already gone,
    // and the demo file, which a record references.
    pend(&store, "99999999-stopped", "attachment_write", "attachments/01890f3e-0000-7000-8000-0000000000aa.png");
    pend(&store, "99999999-stopped", "attachment_write", "attachments/01890f3e-0000-7000-8000-0000000000aa.png.tmp-x");
    pend(&store, "99999999-stopped", "attachment_write", &format!("attachments/{DEMO_ATTACHMENT}"));
    // A write whose owner is still running (process 1 always runs).
    pend(&store, "1-running", "attachment_write", "attachments/01890f3e-0000-7000-8000-0000000000bb.png");
    // A path outside the attachment folder is never acted on.
    pend(&store, "99999999-odd", "attachment_removal", "attachments/../athlete.ath.sqlite");

    let report = resume_pending_file_operations(&mut store).unwrap();
    assert_eq!(report.removed_files, ["attachments/01890f3e-0000-7000-8000-0000000000aa.png"]);
    assert_eq!(report.retained_files, [format!("attachments/{DEMO_ATTACHMENT}")]);
    assert_eq!(report.active_operations, ["1-running"]);
    assert_eq!(report.failed_files.len(), 1);
    assert!(!report.is_complete());
    assert!(attachments.join(DEMO_ATTACHMENT).exists());
    assert!(attachments.join("01890f3e-0000-7000-8000-0000000000bb.png").exists());
    assert!(!attachments.join("01890f3e-0000-7000-8000-0000000000aa.png").exists());
    assert!(database.exists());
    let left: Vec<String> =
        pending_file_operations(store.connection()).unwrap().into_iter().map(|entry| entry.operation_id).collect();
    assert_eq!(left, ["1-running", "99999999-odd"]);
    common::remove_dir(&directory);
}
