//! Helpers for the route golden files in `tests/golden/routes/`.
//!
//! `tools/reference/route-golden.ts` wrote those files from the 2.1.0
//! program. The helpers here build the same inputs and replay the same steps.

#![allow(dead_code)]

pub mod keyed;

use std::io::Write;
use std::path::{Path, PathBuf};

use athleticstandard::import::{self, ImportOptions};
use athleticstandard::json::{self, Value};
use athleticstandard::model::RecordActor;
use athleticstandard::store::{Changes, Deletion, Store};

use crate::import_support::{assert_same_json, new_store, snapshot, KeyFact, Labels, TempDir};

pub fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/routes").join(name);
    json::parse(&std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))).unwrap()
}

pub fn exports() -> PathBuf {
    crate::import_support::repository_root().join("tests/fixtures/exports")
}

/// The bytes of a golden input file: UTF-8 text, or `base64:` bytes.
pub fn bytes_of(text: &str) -> Vec<u8> {
    match text.strip_prefix("base64:") {
        Some(encoded) => base64(encoded),
        None => text.as_bytes().to_vec(),
    }
}

fn base64(text: &str) -> Vec<u8> {
    let value = |byte: u8| -> u32 {
        match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            other => panic!("not base64: {other}"),
        }
    };
    let clean: Vec<u8> = text.bytes().filter(|byte| *byte != b'=').collect();
    let mut out = Vec::new();
    for chunk in clean.chunks(4) {
        let mut bits = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            bits |= value(*byte) << (18 - 6 * index);
        }
        let bytes = bits.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    out
}

fn files(value: &Value) -> Vec<(String, Vec<u8>)> {
    value.as_object().unwrap().iter().map(|(name, text)| (name.clone(), bytes_of(text.as_str().unwrap()))).collect()
}

pub fn write_zip(path: &Path, files: &[(String, Vec<u8>)]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, bytes) in files {
        zip.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// Build a golden step's input under `directory` and return its path.
pub fn materialize(input: &Value, directory: &Path) -> PathBuf {
    match input["kind"].as_str().unwrap() {
        "file" => {
            let path = directory.join(input["name"].as_str().unwrap());
            std::fs::write(&path, bytes_of(input["text"].as_str().unwrap())).unwrap();
            path
        }
        "directory" => {
            let path = directory.join("input");
            std::fs::create_dir_all(&path).unwrap();
            for (name, bytes) in files(&input["files"]) {
                let target = path.join(name);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, bytes).unwrap();
            }
            path
        }
        "zip" => {
            let path = directory.join("input.zip");
            write_zip(&path, &files(&input["files"]));
            path
        }
        other => panic!("unknown input kind {other}"),
    }
}

/// Delete the first record whose label starts with `prefix`.
pub fn delete_by_label(store: &mut Store, labels: &mut Labels, prefix: &str, deleted_at: &str) {
    let before = snapshot(store.connection(), labels);
    let target = before["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["label"].as_str().unwrap().starts_with(prefix))
        .unwrap_or_else(|| panic!("no record to delete: {prefix}"));
    let record_id: String = store
        .connection()
        .prepare("SELECT record_id FROM records")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .find(|id| labels.text(id) == target["label"].as_str().unwrap())
        .unwrap();
    store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id,
                expected_revision: target["current_revision"].as_u64().unwrap(),
                deleted_by: RecordActor::Athlete,
                deleted_at: Some(deleted_at.to_string()),
            }],
            ..Changes::default()
        })
        .unwrap();
}

/// Replay a provider-route scenario (Oura) and require the 2.1.0 results.
pub fn run_provider_scenario(name: &str) {
    let golden = golden(&format!("{name}.json"));
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mut labels = Labels::default();
    for (index, step) in golden["steps"].as_array().unwrap().iter().enumerate() {
        let context = format!("{name} step {index}");
        let step_directory = directory.path().join(format!("step-{index}"));
        std::fs::create_dir(&step_directory).unwrap();
        if let Value::Object(delete) = &step["delete_before"] {
            delete_by_label(
                &mut store,
                &mut labels,
                delete["label_prefix"].as_str().unwrap(),
                delete["deleted_at"].as_str().unwrap(),
            );
        }
        let path = materialize(&step["input"], &step_directory);
        let options = ImportOptions { received_at: Some(step["received_at"].as_str().unwrap().to_string()) };
        let preview = match import::preview(&store, &path, &options) {
            Ok(preview) => preview,
            Err(error) => {
                let expected = step["preview_error"].as_str().unwrap_or_else(|| panic!("{context}: {error}"));
                assert_eq!(error.to_string(), expected, "{context}");
                continue;
            }
        };
        assert!(step.get("preview_error").is_none(), "{context}: the preview should fail");
        labels.learn(
            preview
                .planned()
                .unwrap()
                .into_iter()
                .map(|record| KeyFact {
                    record_id: record.record_id,
                    record_type: record.record_type,
                    method: preview.match_key_method().to_string(),
                    key: record.match_key,
                })
                .collect(),
        );
        let output = labels.value(&json::to_value(&preview));
        assert_same_json(&output, &step["preview"], &format!("{context} preview"));
        let apply = match preview.apply(&mut store) {
            Ok(result) => serde_json::json!({ "result": json::to_value(&result) }),
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        };
        assert_same_json(&apply, &step["apply"], &format!("{context} apply"));
        let database = snapshot(store.connection(), &mut labels);
        assert_same_json(&database, &step["database"], &format!("{context} database"));
    }
}
