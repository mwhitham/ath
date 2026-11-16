//! Privacy erasure: dependency closure, value-free markers, and attachment
//! file removal with recoverable cleanup.
//! Ported from the erasure case of tests/v1-cli-writes.test.ts.
#![cfg_attr(not(unix), allow(unused))]

#[path = "package_support/mod.rs"]
mod common;

use std::fs;

use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::package::*;
use athleticstandard::store::read;
use athleticstandard::Error;
use common::*;

const RESULT_ID: &str = "01890f3e-e7d3-7cc0-88c8-00000000000b";
const ATTACHMENT_ID: &str = "01890f3e-e7d3-7cc0-88c8-00000000000e";
const PREDICTION_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000010";

fn count(store: &athleticstandard::store::Store, record_type: RecordType) -> usize {
    store
        .list_records(&read::ListOptions { record_type: Some(record_type), include_deleted: true, limit: 100 })
        .unwrap()
        .len()
}

#[test]
fn erasure_removes_dependents_and_history_and_leaves_value_free_markers() {
    let directory = temporary("ath-erase-");
    let database = migrated_demo_copy(&directory);
    let mut store = open(&database);
    let preview = preview_erasure(&store, RESULT_ID, 1).unwrap();
    assert_eq!(preview.root_record_id, RESULT_ID);
    assert_eq!(preview.root_expected_revision, 1);
    assert_eq!(preview.database_revision, 3);
    assert_eq!(preview.records[0].record_id, RESULT_ID);
    let types: Vec<RecordType> = preview.records.iter().map(|record| record.record_type).collect();
    assert!(types.contains(&RecordType::PredictionResultLink));
    assert!(types.contains(&RecordType::PredictionGrade));
    assert!(!types.contains(&RecordType::Prediction));
    let preview_json = json::to_value(&preview);
    assert_eq!(
        preview_json["records"][0].as_object().unwrap().keys().collect::<Vec<_>>(),
        ["record_id", "record_type", "current_revision"]
    );
    // A preview writes nothing.
    assert_eq!(count(&store, RecordType::BenchmarkResult), 1);

    let erased = erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, None).unwrap();
    assert_eq!(erased.erased_record_ids.len(), preview.records.len());
    assert_eq!(erased.database_revision, 4);
    assert_eq!(count(&store, RecordType::BenchmarkResult), 0);
    assert_eq!(count(&store, RecordType::PredictionResultLink), 0);
    assert_eq!(count(&store, RecordType::PredictionGrade), 0);
    assert_eq!(count(&store, RecordType::Prediction), 1);
    assert!(store.get_record(PREDICTION_ID).unwrap().is_some());
    for id in &erased.erased_record_ids {
        assert!(store.revisions(id).unwrap().is_empty(), "{id} kept history");
    }
    let marker: (String, String) = direct(store.path())
        .query_row(
            "SELECT erased_at, reason_code FROM record_erasure_markers WHERE record_id = ?1",
            [RESULT_ID],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(marker.1, "athlete_request");
    assert!(time::is_utc_timestamp(&marker.0));

    let output = directory.join("after-erasure");
    export_package(&store, &output, &ExportOptions::default()).unwrap();
    let document = json::parse(&fs::read_to_string(output.join("athlete.ath.json")).unwrap()).unwrap();
    let markers: Vec<&Value> = document["record_erasure_markers"].as_array().unwrap().iter().collect();
    assert!(markers.iter().any(|marker| marker["record_id"] == RESULT_ID));
    for marker in markers {
        assert_eq!(marker.as_object().unwrap().keys().collect::<Vec<_>>(), ["erased_at", "reason_code", "record_id"]);
    }

    let again = preview_erasure(&store, RESULT_ID, 1).unwrap_err();
    assert_eq!(again.to_string(), format!("Record was already erased: {RESULT_ID}"));
    common::remove_dir(&directory);
}

#[test]
fn erasing_an_attachment_removes_its_file_after_the_records() {
    let directory = temporary("ath-erase-attachment-");
    let database = migrated_demo_copy(&directory);
    let file = directory.join("attachments").join(DEMO_ATTACHMENT);
    let mut store = open(&database);
    let preview = preview_erasure(&store, ATTACHMENT_ID, 1).unwrap();
    assert_eq!(preview.attachments.len(), 1);
    assert_eq!(preview.attachments[0].record_id, ATTACHMENT_ID);
    assert_eq!(preview.attachments[0].path, file.display().to_string());
    assert!(preview.attachments[0].present);
    assert!(preview.records.iter().any(|record| record.record_type == RecordType::NutritionEntry));

    let erased = erase_record(&mut store, &preview, ErasureReasonCode::RecordedInError, None).unwrap();
    assert_eq!(erased.erased_attachment_count, 1);
    assert!(!file.exists());
    assert!(store.pending_file_operations().unwrap().is_empty());
    assert!(store.get_record(ATTACHMENT_ID).unwrap().is_none());
    common::remove_dir(&directory);
}

#[test]
fn stale_and_unknown_erasures_are_refused() {
    let directory = temporary("ath-erase-stale-");
    let database = migrated_demo_copy(&directory);
    let mut store = open(&database);
    assert!(matches!(preview_erasure(&store, "not-an-id", 1), Err(Error::Invalid(_))));
    assert_eq!(
        preview_erasure(&store, "01890f3e-e7d3-7cc0-88c8-0000000000aa", 1).unwrap_err().to_string(),
        "Record not found: 01890f3e-e7d3-7cc0-88c8-0000000000aa"
    );
    assert_eq!(
        preview_erasure(&store, RESULT_ID, 2).unwrap_err().to_string(),
        format!("Record {RESULT_ID} is at revision 1, not expected revision 2")
    );
    let preview = preview_erasure(&store, ATTACHMENT_ID, 1).unwrap();
    direct(store.path())
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();
    let error = erase_record(&mut store, &preview, ErasureReasonCode::Other, None).unwrap_err();
    assert_eq!(error.to_string(), "Database changed after the erasure preview; preview it again");
    assert!(store.get_record(ATTACHMENT_ID).unwrap().is_some());
    assert!(directory.join("attachments").join(DEMO_ATTACHMENT).exists());
    assert!(store.pending_file_operations().unwrap().is_empty());
    let bad_time = erase_record(&mut store, &preview, ErasureReasonCode::Other, Some("yesterday"));
    assert!(matches!(bad_time, Err(Error::Invalid(_))));
    common::remove_dir(&directory);
}

#[cfg(unix)]
#[test]
fn a_failed_file_removal_stays_pending_until_a_later_writable_operation() {
    use std::os::unix::fs::PermissionsExt;
    let directory = temporary("ath-erase-pending-");
    let database = migrated_demo_copy(&directory);
    let attachments = directory.join("attachments");
    let file = attachments.join(DEMO_ATTACHMENT);
    let mut store = open(&database);
    let preview = preview_erasure(&store, ATTACHMENT_ID, 1).unwrap();
    fs::set_permissions(&attachments, fs::Permissions::from_mode(0o500)).unwrap();
    let error = erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, None).unwrap_err();
    fs::set_permissions(&attachments, fs::Permissions::from_mode(0o700)).unwrap();
    let message = error.to_string();
    assert!(message.starts_with("erased "), "{message}");
    assert!(message.contains("could not be removed"), "{message}");
    assert!(message.contains("ath check reports it"), "{message}");
    // The records are erased; the file is still there and listed as pending.
    assert!(store.get_record(ATTACHMENT_ID).unwrap().is_none());
    assert!(file.exists());
    store.close().unwrap();

    let reader = open_read_only(&database);
    let pending = reader.pending_file_operations().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_kind, "attachment_removal");
    assert_eq!(pending[0].relative_path, format!("attachments/{DEMO_ATTACHMENT}"));
    drop(reader);
    assert!(file.exists(), "a read-only check must not change files");

    let mut store = open(&database);
    let report = resume_pending_file_operations(&mut store).unwrap();
    assert_eq!(report.removed_files, [format!("attachments/{DEMO_ATTACHMENT}")]);
    assert!(report.is_complete());
    assert!(!file.exists());
    assert!(store.pending_file_operations().unwrap().is_empty());
    common::remove_dir(&directory);
}

#[test]
fn previews_and_an_erasure_match_2_1_0() {
    let directory = temporary("ath-erase-golden-");
    let database = migrated_demo_copy(&directory);
    let mut store = open(&database);
    let expected = json::parse(&fs::read_to_string(golden("erasure").join("previews.json")).unwrap()).unwrap();
    let prefix = format!("{}{}", directory.display(), std::path::MAIN_SEPARATOR);
    for (record_id, expected_preview) in expected.as_object().unwrap() {
        let preview = preview_erasure(&store, record_id, 1).unwrap();
        let mut actual = json::to_value(&preview);
        for attachment in actual["attachments"].as_array_mut().unwrap() {
            // The golden paths come from 2.1.0 on Unix and use `/`.
            let path = attachment["path"].as_str().unwrap().strip_prefix(&prefix).unwrap().replace('\\', "/");
            attachment["path"] = Value::String(path);
        }
        assert_eq!(json::to_pretty(&actual), json::to_pretty(expected_preview), "{record_id}");
    }
    let preview = preview_erasure(&store, ATTACHMENT_ID, 1).unwrap();
    let erased = erase_record(&mut store, &preview, ErasureReasonCode::RecordedInError, Some(EXPORTED_AT)).unwrap();
    let expected_result = fs::read_to_string(golden("erasure").join("erased.json")).unwrap();
    assert_eq!(format!("{}\n", json::to_pretty(&json::to_value(&erased))), expected_result);
    store.close().unwrap();
    let expected_rows = fs::read_to_string(golden("erasure").join("database-rows.txt")).unwrap();
    assert_eq!(dump_rows(&database), expected_rows);
    common::remove_dir(&directory);
}
