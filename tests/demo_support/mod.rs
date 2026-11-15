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
