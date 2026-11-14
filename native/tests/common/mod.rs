//! Shared helpers for the calculation, view, prediction, and log tests.
//!
//! A scenario is a list of steps in the shape the 2.1.0 write functions took
//! (`writeRecords`, `correctRecord`, `deleteRecord`). The reference
//! generators in `native/tools/reference/` replay the same steps through the
//! old implementation and save its outputs as golden files. These helpers
//! replay the steps through the Rust store and compare outputs exactly.

#![allow(dead_code)]

pub mod scenario;

pub use scenario::*;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Map, Value};
use athleticstandard::model::*;
use athleticstandard::store::{Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, Store};

pub const DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-0000000000ff";

pub fn temporary_dir(prefix: &str) -> PathBuf {
    athleticstandard::files::make_temporary_directory(prefix).unwrap()
}

/// A new database in a new temporary directory.
pub fn new_store(created_at: &str) -> (PathBuf, Store) {
    let directory = temporary_dir("ath-test-");
    let path = directory.join("athlete.ath.sqlite");
    let store = Store::create(
        &path,
        CreateOptions { database_id: Some(DATABASE_ID.into()), created_at: Some(created_at.into()) },
    )
    .unwrap();
    (directory, store)
}

fn actor(value: &Value) -> RecordActor {
    serde_json::from_value(value.clone()).expect("actor")
}

fn basis(value: Option<&Value>) -> BecameKnownAtBasis {
    value
        .and_then(Value::as_str)
        .map(|text| BecameKnownAtBasis::parse(text).expect("basis"))
        .unwrap_or(BecameKnownAtBasis::LocalWrite)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A `WriteRecordInput` object as a new record.
pub fn new_record(input: &Value) -> NewRecord {
    let record_type = RecordType::parse(input["record_type"].as_str().unwrap()).unwrap();
    let data = RecordData::decode(record_type, &input["data"])
        .unwrap_or_else(|error| panic!("invalid {record_type} data: {error}"));
    let mut record = NewRecord::new(data, actor(&input["changed_by"]));
    record.record_id = text(input, "record_id");
    record.created_at = text(input, "created_at");
    record.became_known_at = text(input, "became_known_at");
    record.became_known_at_basis = basis(input.get("became_known_at_basis"));
    record.extensions = input.get("extensions").and_then(Value::as_object).cloned();
    record
}

/// Replay scenario steps: `{"write": [...]}`, `{"correct": {...}}`, `{"delete": {...}}`.
pub fn replay(store: &mut Store, steps: &[Value]) {
    for step in steps {
        if let Some(inputs) = step.get("write").and_then(Value::as_array) {
            store.apply(Changes { creates: inputs.iter().map(new_record).collect(), ..Changes::default() }).unwrap();
        } else if let Some(input) = step.get("correct") {
            let record_id = text(input, "record_id").unwrap();
            let current = store.get_record(&record_id).unwrap().unwrap();
            let data = RecordData::decode(current.record_type(), &input["data"]).unwrap();
            store
                .apply(Changes {
                    corrections: vec![Correction {
                        record_id,
                        expected_revision: input["expected_revision"].as_u64().unwrap(),
                        data,
                        extensions: ExtensionsUpdate::Keep,
                        changed_by: actor(&input["changed_by"]),
                        became_known_at: text(input, "became_known_at"),
                        became_known_at_basis: basis(input.get("became_known_at_basis")),
                        stale_style: athleticstandard::store::write::StaleStyle::Revision,
                    }],
                    ..Changes::default()
                })
                .unwrap();
        } else if let Some(input) = step.get("delete") {
            store
                .apply(Changes {
                    deletions: vec![Deletion {
                        record_id: text(input, "record_id").unwrap(),
                        expected_revision: input["expected_revision"].as_u64().unwrap(),
                        deleted_by: actor(&input["deleted_by"]),
                        deleted_at: text(input, "deleted_at"),
                    }],
                    ..Changes::default()
                })
                .unwrap();
        } else {
            panic!("unknown scenario step: {step}");
        }
    }
}

/// Read a golden file under `native/tests/golden/`.
pub fn golden(relative: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    json::parse(&text).unwrap()
}

/// Every record ID a scenario names.
pub fn scenario_ids(steps: &[Value]) -> Vec<String> {
    let mut ids = Vec::new();
    collect_ids(&Value::Array(steps.to_vec()), &mut ids);
    ids
}

fn collect_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::String(text) if is_record_id(text) => {
            if !ids.contains(text) {
                ids.push(text.clone());
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_ids(item, ids)),
        Value::Object(map) => map.values().for_each(|item| collect_ids(item, ids)),
        _ => {}
    }
}

/// Field-specific normalization for old/new comparison:
///
/// - Record IDs the scenario did not name were generated during the call.
///   Each becomes `<generated-N>` in order of first appearance.
/// - `tool_version`, `agent_version`, and `athleticstandard_version` hold the
///   program version, which is 2.1.0 in the reference and 2.1.1 here.
/// - `file` and `report` hold temporary paths.
pub fn normalize(value: &Value, known_ids: &[String]) -> Value {
    let mut generated: HashMap<String, String> = HashMap::new();
    normalize_in(value, known_ids, &mut generated, None)
}

fn normalize_in(value: &Value, known: &[String], generated: &mut HashMap<String, String>, key: Option<&str>) -> Value {
    match value {
        Value::String(text) => {
            if matches!(key, Some("tool_version" | "agent_version" | "athleticstandard_version")) {
                return Value::String("<version>".into());
            }
            if matches!(key, Some("file" | "report")) {
                return Value::String("<path>".into());
            }
            if key == Some("rendered") {
                return Value::String(normalize_text(text, known, generated));
            }
            if is_record_id(text) && !known.contains(text) {
                let next = generated.len() + 1;
                let label = generated.entry(text.clone()).or_insert_with(|| format!("<generated-{next}>"));
                return Value::String(label.clone());
            }
            Value::String(text.clone())
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| normalize_in(item, known, generated, key)).collect())
        }
        Value::Object(map) => {
            let mut out = Map::new();
            for name in json::ordered_keys(map, false) {
                out.insert(name.clone(), normalize_in(&map[name.as_str()], known, generated, Some(name)));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// The same normalization inside human text: generated record IDs and the
/// program version in `"tool_version": "…"` lines.
fn normalize_text(text: &str, known: &[String], generated: &mut HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < text.len() {
        let candidate = text.get(index..index + 36).filter(|part| is_record_id(part));
        match candidate {
            Some(id) if !known.iter().any(|known| known == id) => {
                let next = generated.len() + 1;
                out.push_str(generated.entry(id.to_string()).or_insert_with(|| format!("<generated-{next}>")));
                index += 36;
            }
            _ => {
                let character = text[index..].chars().next().unwrap();
                out.push(character);
                index += character.len_utf8();
            }
        }
    }
    for version in ["2.1.0", ATHLETIC_STANDARD_VERSION] {
        for key in ["tool_version", "agent_version"] {
            out = out.replace(&format!("\"{key}\": \"{version}\""), &format!("\"{key}\": \"<version>\""));
        }
    }
    out
}

/// Assert two JSON values print the same, after normalization.
pub fn assert_same(case: &str, expected: &Value, actual: &Value, known_ids: &[String]) {
    let expected = json::to_pretty(&normalize(expected, known_ids));
    let actual = json::to_pretty(&normalize(actual, known_ids));
    if expected != actual {
        let first = expected.lines().zip(actual.lines()).position(|(left, right)| left != right);
        panic!(
            "{case}: output differs from the 2.1.0 reference (first differing line {:?})\n--- expected\n{expected}\n--- actual\n{actual}",
            first.map(|line| line + 1)
        );
    }
}

/// The result of an operation as golden JSON: the value, or `{"error": message}`.
pub fn outcome<T: serde::Serialize>(result: athleticstandard::Result<T>) -> Value {
    match result {
        Ok(value) => json::to_value(&value),
        Err(error) => {
            let mut map = Map::new();
            map.insert("error".into(), Value::String(error.to_string()));
            Value::Object(map)
        }
    }
}

pub fn string(value: &Value, key: &str) -> Option<String> {
    text(value, key)
}
