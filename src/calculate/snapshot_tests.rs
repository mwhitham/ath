//! A calculation reads every input from one snapshot. A second process
//! deletes an input record while the calculation is between two reads; the
//! result must still equal the result before the deletion.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::{apply, calculate_range, CalculateOptions, Operation};
use crate::json::{self, Value};
use crate::model::{RecordActor, RecordData, RecordType};
use crate::package::failpoints;
use crate::store::{Changes, CreateOptions, Deletion, NewRecord, OpenOptions, Store};

const SOURCE_A: &str = "01890f3e-e7d3-7cc0-88c8-000000000101";
const SOURCE_B: &str = "01890f3e-e7d3-7cc0-88c8-000000000102";

fn tool() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "tool", "tool": "test", "tool_version": "1.0.0" })).unwrap()
}

fn record(record_type: RecordType, id: &str, data: Value) -> NewRecord {
    let mut record = NewRecord::new(RecordData::decode(record_type, &data).unwrap(), tool());
    record.record_id = Some(id.into());
    record.created_at = Some("2026-10-03T00:00:00.000Z".into());
    record
}

fn source(id: &str, writer: &str) -> NewRecord {
    record(RecordType::Source, id, json!({ "source_type": "wearable", "data_writer": writer, "data_route": "test" }))
}

fn point_id(id: u32) -> String {
    format!("01890f3e-e7d3-7cc0-88c8-{id:012x}")
}

fn point(id: u32, source: &str, at: &str, value: f64) -> NewRecord {
    record(
        RecordType::PointMeasurement,
        &point_id(id),
        json!({
            "source_id": source,
            "received_at": "2026-10-03T00:00:00.000Z",
            "received_at_basis": "import_time",
            "quantity": "hrv_rmssd",
            "recorded_at": at,
            "input_timestamp": {
                "timestamp_text": at,
                "utc_offset_text": "Z",
                "utc_offset_meaning": "documented_recording_offset",
                "timestamp_basis": "source_instant",
            },
            "value": { "standard_value": value, "standard_unit": "ms", "input_value": value, "input_unit": "ms" },
        }),
    )
}

/// Two wearable sources with two readings each.
fn database() -> (PathBuf, PathBuf) {
    let directory = crate::files::make_temporary_directory("ath-calculation-snapshot-").unwrap();
    let path = directory.join("athlete.ath.sqlite");
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    store
        .apply(Changes {
            creates: vec![
                source(SOURCE_A, "Device A"),
                source(SOURCE_B, "Device B"),
                point(0x201, SOURCE_A, "2026-10-01T12:00:00.000Z", 60.0),
                point(0x202, SOURCE_A, "2026-10-02T12:00:00.000Z", 70.0),
                point(0x203, SOURCE_B, "2026-10-01T12:00:00.000Z", 50.0),
                point(0x204, SOURCE_B, "2026-10-02T12:00:00.000Z", 55.0),
            ],
            ..Changes::default()
        })
        .unwrap();
    store.close().unwrap();
    (directory, path)
}

/// Delete one record through a second store, as another process would.
fn delete_elsewhere(path: &Path, record_id: &str) {
    let mut other = Store::open(path, OpenOptions::default()).unwrap();
    other
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: record_id.into(),
                expected_revision: 1,
                deleted_by: tool(),
                deleted_at: None,
            }],
            ..Changes::default()
        })
        .unwrap();
    other.close().unwrap();
}

/// Run `hook` the first time the calculation reaches `point`.
fn once_at(point: &str, path: &Path, record_id: &str) {
    let path = path.to_path_buf();
    let record_id = record_id.to_string();
    let mut done = false;
    failpoints::on(point, move || {
        if !done {
            done = true;
            delete_elsewhere(&path, &record_id);
        }
    });
}

#[test]
fn a_range_calculation_reads_every_input_from_one_snapshot() {
    let (directory, path) = database();
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let mut options = CalculateOptions::new(Operation::CompareSources, "hrv_rmssd");
    options.source_id = Some(SOURCE_A.into());
    options.compare_source_id = Some(SOURCE_B.into());
    let before = json::encode(&json::to_value(&calculate_range(&store, &options).unwrap()));

    // Source A's readings are read first. Before source B's are read, another
    // process deletes one of them and commits.
    once_at("calculate.after_inputs", &path, &point_id(0x203));
    let during = calculate_range(&store, &options);
    failpoints::disarm_all();
    assert_eq!(json::encode(&json::to_value(&during.unwrap())), before);

    // The deletion committed: the next calculation sees it.
    let after = calculate_range(&store, &options).unwrap();
    let super::CalculationResult::CompareSources(result) = after.result else { panic!("compare sources") };
    assert_eq!(result.comparison_source.summary.mean, 55.0);
    store.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn apply_reads_the_record_and_its_earlier_records_from_one_snapshot() {
    let (directory, path) = database();
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let before = json::encode(&json::to_value(&apply(&store, "hrv-change", &point_id(0x202), &directory).unwrap()));

    // The target is read first. Before its earlier reading from the same
    // source is read, another process deletes that reading and commits.
    once_at("calculate.apply_after_target", &path, &point_id(0x201));
    let during = apply(&store, "hrv-change", &point_id(0x202), &directory);
    failpoints::disarm_all();
    assert_eq!(json::encode(&json::to_value(&during.unwrap())), before);

    let after = json::encode(&json::to_value(&apply(&store, "hrv-change", &point_id(0x202), &directory).unwrap()));
    assert_ne!(after, before, "the deletion is seen once it has committed");
    store.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_snapshot_read_ends_its_transaction_after_an_error() {
    let (directory, path) = database();
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let error = store.read_snapshot(|| -> crate::Result<()> { Err(crate::Error::other("stop")) }).unwrap_err();
    assert_eq!(error.to_string(), "stop");
    assert!(store.connection().is_autocommit());
    store.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
