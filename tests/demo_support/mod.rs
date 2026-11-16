//! The deterministic demo database in `examples/demo-athlete/`.
//!
//! `history.json` holds the fixed IDs, times, and records, in the order they
//! are written: one batch that creates every record type, one correction, and
//! one deletion. They are the records the 2.1.0 generator wrote, so the 2.1.1
//! demo differs from `tests/fixtures/version-2-1-0/` only in its version
//! metadata and SQL schema step.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::files;
use athleticstandard::json::{self, Value};
use athleticstandard::model::{BecameKnownAtBasis, RecordActor, RecordData, RecordType};
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, Store};

pub const DATABASE: &str = "athlete.ath.sqlite";
pub const ATTACHMENTS: &str = "attachments";

fn history() -> Value {
    json::parse(include_str!("history.json")).expect("history.json is JSON")
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_else(|| panic!("{key} is a string")).to_string()
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).map(|field| field.as_str().unwrap_or_else(|| panic!("{key} is a string")).to_string())
}

fn basis(value: &Value) -> BecameKnownAtBasis {
    match value.get("became_known_at_basis") {
        Some(field) => BecameKnownAtBasis::parse(field.as_str().expect("basis is a string")).expect("a known basis"),
        None => BecameKnownAtBasis::LocalWrite,
    }
}

fn actor(value: &Value) -> RecordActor {
    serde_json::from_value(value.clone()).expect("a record actor")
}

fn data(record_type: RecordType, value: &Value) -> RecordData {
    RecordData::decode(record_type, &value["data"]).unwrap_or_else(|error| panic!("{record_type}: {error:?}"))
}

/// The demo's create entry for one record: `record_type`, `record_id`,
/// `created_at`, `changed_by`, and `data`.
pub fn created(record_id: &str) -> Value {
    history()["changes"][0]["creates"]
        .as_array()
        .expect("the first change creates records")
        .iter()
        .find(|create| create["record_id"] == record_id)
        .unwrap_or_else(|| panic!("the demo has no record {record_id}"))
        .clone()
}

/// The attachment file name and bytes.
pub fn attachment() -> (String, Vec<u8>) {
    let attachment = &history()["attachment"];
    let hex = text(attachment, "hex");
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex digits"))
        .collect();
    (text(attachment, "file"), bytes)
}

/// Write the demo database and its attachment into the empty folder `directory`.
pub fn write(directory: &Path) -> PathBuf {
    let history = history();
    let attachments = directory.join(ATTACHMENTS);
    files::create_private_dir_all(&attachments).unwrap();
    let (name, bytes) = attachment();
    let attachment_path = attachments.join(name);
    fs::write(&attachment_path, bytes).unwrap();
    files::set_owner_only(&attachment_path).unwrap();

    let path = directory.join(DATABASE);
    let database = &history["database"];
    let mut store = Store::create(
        &path,
        CreateOptions {
            database_id: Some(text(database, "database_id")),
            created_at: Some(text(database, "created_at")),
        },
    )
    .unwrap();

    for step in history["changes"].as_array().expect("changes is an array") {
        let mut changes = Changes::default();
        for create in step.get("creates").and_then(Value::as_array).into_iter().flatten() {
            let record_type = RecordType::parse(&text(create, "record_type")).expect("a record type");
            changes.creates.push(NewRecord {
                data: data(record_type, create),
                extensions: None,
                record_id: Some(text(create, "record_id")),
                created_at: Some(text(create, "created_at")),
                became_known_at: optional_text(create, "became_known_at"),
                became_known_at_basis: basis(create),
                changed_by: actor(&create["changed_by"]),
            });
        }
        for correction in step.get("corrections").and_then(Value::as_array).into_iter().flatten() {
            let record_id = text(correction, "record_id");
            let record_type = store.get_record(&record_id).unwrap().expect("the corrected record exists").record_type();
            changes.corrections.push(Correction {
                record_id,
                expected_revision: correction["expected_revision"].as_u64().expect("a revision"),
                data: data(record_type, correction),
                extensions: ExtensionsUpdate::Keep,
                changed_by: actor(&correction["changed_by"]),
                became_known_at: optional_text(correction, "became_known_at"),
                became_known_at_basis: basis(correction),
                stale_style: StaleStyle::Revision,
            });
        }
        for deletion in step.get("deletions").and_then(Value::as_array).into_iter().flatten() {
            changes.deletions.push(Deletion {
                record_id: text(deletion, "record_id"),
                expected_revision: deletion["expected_revision"].as_u64().expect("a revision"),
                deleted_by: actor(&deletion["deleted_by"]),
                deleted_at: optional_text(deletion, "deleted_at"),
            });
        }
        store.apply(changes).unwrap();
    }
    store.close().unwrap();
    path
}

// ---- The demo with invented series ----------------------------------------

/// The demo's wearable source.
pub const SOURCE_A: &str = "01890f3e-e7d3-7cc0-88c8-000000000003";
/// A second wearable source added by [`write_with_series`].
pub const SOURCE_B: &str = "01890f3e-e7d3-7cc0-88c8-000000000064";
/// The demo's workout session, 2026-08-10 17:25 to 17:48 UTC.
pub const WORKOUT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000a";
/// The days of the added daily series.
pub const SERIES_DAYS: [&str; 5] = ["2026-08-01", "2026-08-02", "2026-08-03", "2026-08-04", "2026-08-05"];
/// Source A's daily `hrv_sdnn` values, in ms.
pub const SDNN_A: [f64; 5] = [40.0, 42.0, 44.0, 46.0, 48.0];
/// Source B's daily `hrv_sdnn` values: source A's less 2 ms.
pub const SDNN_B: [f64; 5] = [38.0, 40.0, 42.0, 44.0, 46.0];
/// Source A's daily `respiratory_rate` values, in breaths per minute, falling
/// by the same step as SDNN rises.
pub const RESPIRATORY_A: [f64; 5] = [16.0, 15.5, 15.0, 14.5, 14.0];
/// Source A's heart rate inside the workout: three samples before its
/// midpoint (17:36:30) and three after.
pub const WORKOUT_HEART_RATE: [(&str, f64); 6] = [
    ("2026-08-10T17:26:00.000Z", 140.0),
    ("2026-08-10T17:30:00.000Z", 142.0),
    ("2026-08-10T17:34:00.000Z", 144.0),
    ("2026-08-10T17:38:00.000Z", 150.0),
    ("2026-08-10T17:42:00.000Z", 152.0),
    ("2026-08-10T17:46:00.000Z", 154.0),
];
const SERIES_CREATED_AT: &str = "2026-08-12T00:00:00.000Z";

/// The ID of the `index`th added record.
pub fn series_id(index: usize) -> String {
    format!("01890f3e-e7d3-7cc0-88c8-{:012x}", 0x100 + index)
}

fn sample(source: &str, quantity: &str, recorded_at: &str, value: f64, unit: &str) -> Value {
    serde_json::json!({
        "source_id": source,
        "received_at": SERIES_CREATED_AT,
        "received_at_basis": "import_time",
        "quantity": quantity,
        "recorded_at": recorded_at,
        "input_timestamp": {
            "timestamp_text": recorded_at,
            "utc_offset_text": "Z",
            "utc_offset_meaning": "documented_recording_offset",
            "timestamp_basis": "source_instant"
        },
        "value": { "standard_value": value, "standard_unit": unit, "input_value": value, "input_unit": unit },
        "field_basis": { "value": "provider" }
    })
}

/// The demo database with a second wearable source and invented series:
/// daily `hrv_sdnn` for both sources, daily `respiratory_rate` for source A,
/// and source A heart rate inside the demo workout.
pub fn write_with_series(directory: &Path) -> PathBuf {
    let path = write(directory);
    let tool = || actor(&serde_json::json!({ "author_type": "tool", "tool": "ath", "tool_version": "1.0.0" }));
    let mut records = vec![(
        RecordType::Source,
        serde_json::json!({ "source_type": "wearable", "data_writer": "Example Band", "data_route": "provider_export" }),
        Some(SOURCE_B.to_string()),
    )];
    for (index, day) in SERIES_DAYS.iter().enumerate() {
        let at = format!("{day}T06:00:00.000Z");
        records.push((RecordType::MeasurementSample, sample(SOURCE_A, "hrv_sdnn", &at, SDNN_A[index], "ms"), None));
        records.push((RecordType::MeasurementSample, sample(SOURCE_B, "hrv_sdnn", &at, SDNN_B[index], "ms"), None));
        records.push((
            RecordType::MeasurementSample,
            sample(SOURCE_A, "respiratory_rate", &at, RESPIRATORY_A[index], "brpm"),
            None,
        ));
    }
    for (at, value) in WORKOUT_HEART_RATE {
        records.push((RecordType::MeasurementSample, sample(SOURCE_A, "heart_rate", at, value, "bpm"), None));
    }
    let creates = records
        .into_iter()
        .enumerate()
        .map(|(index, (record_type, data, record_id))| NewRecord {
            data: RecordData::decode(record_type, &data).unwrap_or_else(|error| panic!("{record_type}: {error:?}")),
            extensions: None,
            record_id: Some(record_id.unwrap_or_else(|| series_id(index))),
            created_at: Some(SERIES_CREATED_AT.to_string()),
            became_known_at: None,
            became_known_at_basis: BecameKnownAtBasis::ProviderImport,
            changed_by: tool(),
        })
        .collect();
    let mut store = Store::open(&path, athleticstandard::store::OpenOptions::default()).unwrap();
    store.apply(Changes { creates, ..Changes::default() }).unwrap();
    store.close().unwrap();
    path
}
