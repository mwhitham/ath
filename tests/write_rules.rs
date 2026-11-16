//! Write-time rules the store enforces before or at commit. Each refusal
//! leaves the database revision and every record unchanged.
//!
//! These are the `tests/v1-db.test.ts` promises from 2.1.0 that the rest of
//! the native suite checked only at `ath check` time or through raw SQL:
//! typed prediction-ledger references, matching benchmarks, exact grade
//! arithmetic, knowledge time that cannot go backward, references to
//! attachments, calculation inputs, and goal targets, one-transaction batches,
//! symbolic-link refusal, and owner-only database files.

mod demo_support;
#[path = "temporary_root.rs"]
mod temporary_root;

use std::path::PathBuf;

use athleticstandard::json::{self, Value};
use athleticstandard::model::{BecameKnownAtBasis, RecordActor, RecordData, RecordType};
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{Changes, Correction, Deletion, ExtensionsUpdate, NewRecord, OpenOptions, Store};
use serde_json::json;

const MANUAL_SOURCE: &str = "01890f3e-e7d3-7cc0-88c8-000000000004";
const FRAN: &str = "01890f3e-e7d3-7cc0-88c8-000000000006";
const POINT: &str = "01890f3e-e7d3-7cc0-88c8-000000000007";
const RESULT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000b";
const PREDICTION: &str = "01890f3e-e7d3-7cc0-88c8-000000000010";
const GRADE: &str = "01890f3e-e7d3-7cc0-88c8-000000000012";
const MISSING: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1eee";
const NOW: &str = "2026-08-12T12:00:00.000Z";
/// SQLite checks these references when the write transaction commits.
const FOREIGN_KEY: &str = "FOREIGN KEY constraint failed";

/// A writable copy of the demo database.
fn demo_store() -> (PathBuf, Store) {
    let directory = temporary_root::new_dir("ath-write-rules-");
    let path = demo_support::write(&directory);
    let store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: false }).unwrap();
    (directory, store)
}

fn tool() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "tool", "tool": "ath", "tool_version": "1.0.0" })).unwrap()
}

fn new_record(record_type: &str, record_id: &str, data: Value) -> NewRecord {
    let record_type = RecordType::parse(record_type).unwrap();
    let data = RecordData::decode(record_type, &data).unwrap_or_else(|error| panic!("{record_type}: {error:?}"));
    NewRecord {
        record_id: Some(record_id.to_string()),
        created_at: Some(NOW.to_string()),
        ..NewRecord::new(data, tool())
    }
}

fn creates(records: Vec<NewRecord>) -> Changes {
    Changes { creates: records, ..Changes::default() }
}

fn database_revision(store: &Store) -> i64 {
    store.database_revision().unwrap() as i64
}

fn record_exists(store: &Store, record_id: &str) -> bool {
    store.get_record(record_id).unwrap().is_some()
}

/// Apply `changes`, require a refusal that names `expected`, and require that
/// nothing changed.
fn refused(store: &mut Store, changes: Changes, expected: &str) {
    let before = database_revision(store);
    let new_ids: Vec<String> = changes.creates.iter().filter_map(|record| record.record_id.clone()).collect();
    let error = store.apply(changes).expect_err("the write is refused").to_string();
    assert!(error.contains(expected), "expected {expected:?} in {error:?}");
    assert_eq!(database_revision(store), before, "a refused write changed the database revision");
    for record_id in new_ids {
        assert!(!record_exists(store, &record_id), "{record_id} was written by a refused change");
    }
}

/// The demo's data for one record, changed by `edit`.
fn demo_data(record_id: &str, edit: impl FnOnce(&mut Value)) -> Value {
    let mut data = demo_support::created(record_id)["data"].clone();
    edit(&mut data);
    data
}

fn note(attachment_ids: Option<Vec<&str>>) -> Value {
    let mut data = json!({
        "report_type": "soreness",
        "reported_at": NOW,
        "input_reported_at": {
            "timestamp_missing_reason": "not_provided_by_source",
            "utc_offset_missing_reason": "not_provided_by_source"
        },
        "reported_via": "text"
    });
    if let Some(ids) = attachment_ids {
        data["attachment_ids"] = json!(ids);
    }
    data
}

#[test]
fn a_link_must_name_a_prediction_and_a_benchmark_result() {
    let (_directory, mut store) = demo_store();
    let link = json!({
        "prediction": { "record_id": RESULT, "revision": 1 },
        "benchmark_result": { "record_id": RESULT, "revision": 1 },
        "created_by": { "author_type": "tool", "tool": "ath", "tool_version": "1.0.0" },
        "linked_at": NOW,
        "content_version": 1,
        "status": "active"
    });
    let changes = creates(vec![new_record("prediction_result_link", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e01", link)]);
    refused(&mut store, changes, &format!("Referenced {RESULT}@1 is benchmark_result, not prediction"));
}

#[test]
fn a_link_must_join_one_benchmark_and_a_grade_must_match_its_arithmetic() {
    let (_directory, mut store) = demo_store();
    let murph = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
    let murph_result = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e03";
    let definition = demo_data(FRAN, |data| {
        data["benchmark_key"] = json!("murph");
        data["definition"] = json!("1 mile run, 100 pull-ups, 200 push-ups, 300 squats, 1 mile run");
    });
    let result = demo_data(RESULT, |data| {
        data["benchmark_definition_id"] = json!(murph);
        data["score"] = json!({ "duration_s": 2700 });
        data.as_object_mut().unwrap().remove("workout_session_id");
    });
    store
        .apply(creates(vec![
            new_record("benchmark_definition", murph, definition),
            new_record("benchmark_result", murph_result, result),
        ]))
        .unwrap();

    let link = json!({
        "prediction": { "record_id": PREDICTION, "revision": 1 },
        "benchmark_result": { "record_id": murph_result, "revision": 1 },
        "created_by": { "author_type": "tool", "tool": "ath", "tool_version": "1.0.0" },
        "linked_at": NOW,
        "content_version": 1,
        "status": "active"
    });
    let changes = creates(vec![new_record("prediction_result_link", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e04", link)]);
    refused(&mut store, changes, "Prediction and benchmark result use different benchmark definitions");

    let wrong_arithmetic = demo_data(GRADE, |data| data["signed_error"] = json!(-10));
    let changes =
        creates(vec![new_record("prediction_grade", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e05", wrong_arithmetic)]);
    refused(&mut store, changes, "Saved grade arithmetic does not match its linked revisions");

    let wrong_prediction = demo_data(GRADE, |data| data["predicted_score"] = json!({ "duration_s": 281 }));
    let changes =
        creates(vec![new_record("prediction_grade", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e06", wrong_prediction)]);
    refused(&mut store, changes, "Saved grade predicted score does not match the linked prediction revision");

    // The demo's own grade arithmetic is accepted when written again.
    let copy = demo_data(GRADE, |_| {});
    store.apply(creates(vec![new_record("prediction_grade", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e07", copy)])).unwrap();
}

#[test]
fn knowledge_time_cannot_go_backward() {
    let (_directory, mut store) = demo_store();
    // The demo's point measurement is at revision 2, known at 2026-08-10T12:00.
    let current = store.get_record(POINT).unwrap().unwrap();
    let correction = Correction {
        record_id: POINT.to_string(),
        expected_revision: 2,
        data: current.data.clone(),
        extensions: ExtensionsUpdate::Keep,
        changed_by: tool(),
        became_known_at: Some("2026-08-10T11:59:59.999Z".to_string()),
        became_known_at_basis: BecameKnownAtBasis::ProviderImport,
        stale_style: StaleStyle::Revision,
    };
    refused(
        &mut store,
        Changes { corrections: vec![correction], ..Changes::default() },
        "Revision knowledge time cannot precede revision 2",
    );
    let deletion = Deletion {
        record_id: POINT.to_string(),
        expected_revision: 2,
        deleted_by: tool(),
        deleted_at: Some("2026-08-10T11:00:00.000Z".to_string()),
    };
    refused(
        &mut store,
        Changes { deletions: vec![deletion], ..Changes::default() },
        "Deletion time cannot precede the current revision knowledge time",
    );
    assert_eq!(store.get_record(POINT).unwrap().unwrap().current_revision, 2);
}

#[test]
fn references_to_missing_attachments_inputs_and_benchmarks_are_refused() {
    let (_directory, mut store) = demo_store();
    let report = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e10";
    let changes = creates(vec![new_record("athlete_report", report, note(Some(vec![MISSING])))]);
    refused(&mut store, changes, FOREIGN_KEY);

    let measurement = demo_data(POINT, |data| {
        data["calculations"] = json!([{
            "from_quantity": "hrv_beats",
            "method": "rmssd",
            "calculation_version": "1",
            "input_records": [{ "record_id": MISSING, "revision": 1 }]
        }]);
        data["provider_record_id"] = json!("calculated-rmssd");
    });
    let changes = creates(vec![new_record("point_measurement", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e11", measurement)]);
    refused(&mut store, changes, FOREIGN_KEY);

    let goal = json!({
        "authorship": {
            "author": { "author_type": "athlete" },
            "authored_at": NOW,
            "evidence": [],
            "content_version": 1,
            "status": "active"
        },
        "title": "Improve Fran",
        "target_benchmark_definition_id": MISSING,
        "target_score": { "duration_s": 240 }
    });
    refused(&mut store, creates(vec![new_record("goal", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e12", goal)]), FOREIGN_KEY);
}

#[test]
fn a_batch_with_one_missing_source_writes_nothing() {
    let (_directory, mut store) = demo_store();
    let report = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e20";
    let measurement = demo_data(POINT, |data| {
        data["source_id"] = json!(MISSING);
        data["provider_record_id"] = json!("from-a-missing-source");
    });
    let changes = creates(vec![
        new_record("athlete_report", report, note(None)),
        new_record("point_measurement", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21", measurement),
    ]);
    refused(&mut store, changes, FOREIGN_KEY);

    // The same batch with a real source is one database revision.
    let before = database_revision(&store);
    let measurement = demo_data(POINT, |data| {
        data["source_id"] = json!(MANUAL_SOURCE);
        data["provider_record_id"] = json!("from-the-manual-source");
    });
    store
        .apply(creates(vec![
            new_record("athlete_report", report, note(None)),
            new_record("point_measurement", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21", measurement),
        ]))
        .unwrap();
    assert_eq!(database_revision(&store), before + 1);
    assert!(record_exists(&store, report));
}

#[test]
fn a_17_digit_coordinate_survives_a_correction_and_the_check() {
    let (_directory, mut store) = demo_store();
    let precise = 14.384010205470487_f64;
    let timestamp = |text: &str| {
        json!({
            "timestamp_text": text,
            "utc_offset_text": "Z",
            "utc_offset_meaning": "documented_recording_offset",
            "timestamp_basis": "source_instant"
        })
    };
    let workout = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e50";
    let mut data = json!({
        "source_id": MANUAL_SOURCE,
        "received_at": "2026-08-09T16:00:00.000Z",
        "received_at_basis": "import_time",
        "start_at": "2026-08-09T15:00:00.000Z",
        "end_at": "2026-08-09T15:10:00.000Z",
        "input_start_timestamp": timestamp("2026-08-09T15:00:00.000Z"),
        "input_end_timestamp": timestamp("2026-08-09T15:10:00.000Z"),
        "source_reported_summaries": [],
        "route": [{ "recorded_at": "2026-08-09T15:00:00.000Z", "latitude_deg": precise, "longitude_deg": -105 }]
    });
    store.apply(creates(vec![new_record("workout_session", workout, data.clone())])).unwrap();
    let check = |store: &Store| athleticstandard::store::check::check_database(store).unwrap();
    assert!(check(&store).valid, "{:?}", check(&store).issues);

    data["maximum_heart_rate_bpm"] = json!(190);
    let decoded = RecordData::decode(RecordType::WorkoutSession, &data).unwrap();
    let correction = Correction {
        record_id: workout.to_string(),
        expected_revision: 1,
        data: decoded,
        extensions: ExtensionsUpdate::Keep,
        changed_by: tool(),
        became_known_at: Some(NOW.to_string()),
        became_known_at_basis: BecameKnownAtBasis::LocalWrite,
        stale_style: StaleStyle::Revision,
    };
    store.apply(Changes { corrections: vec![correction], ..Changes::default() }).unwrap();
    assert!(check(&store).valid, "{:?}", check(&store).issues);
    let stored = json::parse(&store.get_record(workout).unwrap().unwrap().data.to_json()).unwrap();
    assert_eq!(stored["route"][0]["latitude_deg"].as_f64(), Some(precise));
    assert!(store.get_record(workout).unwrap().unwrap().data.to_json().contains("14.384010205470487"));
}

#[cfg(unix)]
mod unix {
    use super::*;
    use athleticstandard::store::CreateOptions;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::Path;

    fn mode(path: &Path) -> u32 {
        std::fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_database_and_its_sidecars_are_owner_only() {
        let directory = temporary_root::new_dir("ath-write-rules-mode-");
        let path = directory.join("athlete.ath.sqlite");
        let mut store = Store::create(&path, CreateOptions::default()).unwrap();
        let source = json!({ "source_type": "manual", "data_writer": "ath", "data_route": "manual_entry" });
        store.apply(creates(vec![new_record("source", "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e30", source)])).unwrap();
        for suffix in ["", "-wal", "-shm"] {
            let file = PathBuf::from(format!("{}{suffix}", path.display()));
            assert!(file.exists(), "{} exists while the store is open", file.display());
            assert_eq!(mode(&file), 0o600, "{}", file.display());
        }
        store.close().unwrap();
        assert_eq!(mode(&path), 0o600);
    }

    /// The refusal message of a store operation that must fail.
    fn refusal(result: athleticstandard::Result<Store>, what: &str) -> String {
        match result {
            Ok(_) => panic!("{what} was not refused"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn a_database_reached_through_a_symbolic_link_is_refused() {
        let directory = temporary_root::new_dir("ath-write-rules-link-");
        let path = demo_support::write(&directory.join("real"));
        let file_link = directory.join("link.ath.sqlite");
        symlink(&path, &file_link).unwrap();
        let read_only = OpenOptions { read_only: true, allow_outdated_schema: false };
        let error = refusal(Store::open(&file_link, read_only), "opening through a link");
        assert!(error.contains("symbolic link"), "{error}");

        let folder_link = directory.join("linked-folder");
        symlink(directory.join("real"), &folder_link).unwrap();
        let error =
            refusal(Store::open(folder_link.join("athlete.ath.sqlite"), OpenOptions::default()), "a linked folder");
        assert!(error.contains("symbolic link"), "{error}");

        let error = refusal(Store::create(folder_link.join("new.ath.sqlite"), CreateOptions::default()), "creating");
        assert!(error.contains("symbolic link"), "{error}");
        assert!(!directory.join("real/new.ath.sqlite").exists());
    }
}

#[test]
fn the_demo_store_accepts_a_valid_note() {
    let (_directory, mut store) = demo_store();
    let before = database_revision(&store);
    let report = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e40";
    store.apply(creates(vec![new_record("athlete_report", report, note(None))])).unwrap();
    assert_eq!(database_revision(&store), before + 1);
    let stored = store.get_record(report).unwrap().unwrap();
    assert_eq!(stored.current_revision, 1);
    assert_eq!(json::parse(&stored.data.to_json()).unwrap(), note(None));
}
