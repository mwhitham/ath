//! The 2.1.2 record additions (D238, D239): the `session_effort` report type,
//! the report's workout link, and the strength-set movement and load basis.
//! Each rule is checked by the model, by SQLite, and through erasure.

mod demo_support;
#[path = "temporary_root.rs"]
mod temporary_root;

use std::path::PathBuf;

use athleticstandard::json::Value;
use athleticstandard::model::{registry, ErasureReasonCode, RecordActor, RecordData, RecordType};
use athleticstandard::package::{erase_record, preview_erasure};
use athleticstandard::store::{Changes, NewRecord, OpenOptions, Store};
use serde_json::json;

const WORKOUT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const REPORT: &str = "01890f3e-e7d3-7cc0-88c8-0000000000e1";
const MISSING: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1eee";
const NOW: &str = "2026-08-12T12:00:00.000Z";

fn demo_store() -> (PathBuf, Store) {
    let directory = temporary_root::new_dir("ath-record-2-1-2-");
    let path = demo_support::write(&directory);
    let store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: false }).unwrap();
    (directory, store)
}

fn athlete() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "athlete" })).unwrap()
}

fn issues(schema: &str, value: serde_json::Value) -> Vec<String> {
    let value: Value = athleticstandard::json::parse(&value.to_string()).unwrap();
    registry::shape(schema).unwrap().issues(&value).iter().map(|issue| issue.to_json().to_string()).collect()
}

fn effort(extra: serde_json::Value) -> serde_json::Value {
    let mut data = json!({
        "report_type": "session_effort",
        "reported_at": NOW,
        "input_reported_at": {
            "timestamp_text": NOW,
            "utc_offset_text": "Z",
            "utc_offset_meaning": "documented_recording_offset",
            "timestamp_basis": "source_instant"
        },
        "reported_via": "text",
        "rating": 7,
        "rating_scale": "0-10",
        "duration_seconds": 3600
    });
    for (key, value) in extra.as_object().unwrap() {
        if value.is_null() {
            data.as_object_mut().unwrap().remove(key);
        } else {
            data[key] = value.clone();
        }
    }
    data
}

fn report_record(data: serde_json::Value) -> NewRecord {
    let value: Value = athleticstandard::json::parse(&data.to_string()).unwrap();
    let data = RecordData::decode(RecordType::AthleteReport, &value).unwrap();
    NewRecord {
        record_id: Some(REPORT.to_string()),
        created_at: Some(NOW.to_string()),
        ..NewRecord::new(data, athlete())
    }
}

#[test]
fn a_session_effort_needs_a_rating_and_a_scale() {
    assert!(issues("AthleteReportData", effort(json!({}))).is_empty());
    let without_rating = issues("AthleteReportData", effort(json!({ "rating": null, "rating_scale": null })));
    assert_eq!(without_rating.len(), 1, "{without_rating:?}");
    assert!(without_rating[0].contains("A session effort report needs a rating"), "{without_rating:?}");
    let without_scale = issues("AthleteReportData", effort(json!({ "rating_scale": null })));
    assert!(without_scale[0].contains("A rating needs its scale"), "{without_scale:?}");
    // Other report types still need no rating.
    assert!(issues(
        "AthleteReportData",
        effort(json!({ "report_type": "note", "rating": null, "rating_scale": null }))
    )
    .is_empty());
}

#[test]
fn a_per_implement_load_needs_its_count_and_only_then() {
    let set = |extra: serde_json::Value| {
        let mut set = json!({ "reps": 10, "weight": 22.5, "weight_unit": "kg" });
        for (key, value) in extra.as_object().unwrap() {
            set[key] = value.clone();
        }
        issues("StrengthSet", set)
    };
    assert!(set(json!({})).is_empty());
    assert!(set(json!({ "movement": "dumbbell bench press", "load_basis": "per_implement", "implement_count": 2 }))
        .is_empty());
    assert!(set(json!({ "load_basis": "total" })).is_empty());
    assert!(set(json!({ "load_basis": "per_implement" }))[0].contains("A per-implement load needs implement_count"));
    assert!(set(json!({ "load_basis": "total", "implement_count": 2 }))[0]
        .contains("implement_count is used only with a per_implement load_basis"));
    assert!(set(json!({ "implement_count": 2 }))[0].contains("implement_count is used only"));
    assert!(!set(json!({ "load_basis": "both" })).is_empty());
    assert!(!set(json!({ "movement": "" })).is_empty());
}

#[test]
fn a_report_links_to_its_workout_and_sqlite_enforces_the_link() {
    let (directory, mut store) = demo_store();
    let before = store.database_revision().unwrap();
    let error = store
        .apply(Changes {
            creates: vec![report_record(effort(json!({ "workout_session_id": MISSING })))],
            ..Changes::default()
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("FOREIGN KEY constraint failed"), "{error}");
    assert_eq!(store.database_revision().unwrap(), before, "a refused link writes nothing");

    store
        .apply(Changes {
            creates: vec![report_record(effort(json!({ "workout_session_id": WORKOUT })))],
            ..Changes::default()
        })
        .unwrap();
    let linked: Option<String> = rusqlite::Connection::open(store.path())
        .unwrap()
        .query_row("SELECT workout_session_id FROM athlete_reports WHERE record_id = ?1", [REPORT], |row| row.get(0))
        .unwrap();
    assert_eq!(linked.as_deref(), Some(WORKOUT));

    // Erasing the workout erases the report that names it (D107).
    let preview = preview_erasure(&store, WORKOUT, 1).unwrap();
    assert!(preview.records.iter().any(|record| record.record_id == REPORT), "{preview:?}");
    erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, None).unwrap();
    assert!(store.get_record(REPORT).unwrap().is_none());
    assert!(store.get_record(WORKOUT).unwrap().is_none());
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}
