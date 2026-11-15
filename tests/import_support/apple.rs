//! Replaying the Apple golden files written by
//! `tools/reference/import-golden-apple.ts`.
//!
//! A step's files are text, or `{ "fixture": path }` for an approved fixture
//! whose bytes are copied from the repository. A folder input may name the
//! file to open inside it, such as a bare `export.xml` beside its folders.

use std::path::{Path, PathBuf};

use athleticstandard::import::{self, ImportOptions};
use athleticstandard::json::{self, Value};
use athleticstandard::model::RecordActor;
use athleticstandard::store::{Changes, Deletion};

use super::{assert_same_json, direct, golden, new_store, repository_root, snapshot, KeyFact, Labels, TempDir};

fn file_bytes(body: &Value) -> Vec<u8> {
    match body {
        Value::String(text) => text.as_bytes().to_vec(),
        Value::Object(map) => {
            let path = map["fixture"].as_str().expect("a fixture path");
            std::fs::read(repository_root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
        }
        other => panic!("unknown file body {other}"),
    }
}

fn files(input: &Value) -> Vec<(String, Vec<u8>)> {
    input["files"].as_object().unwrap().iter().map(|(name, body)| (name.clone(), file_bytes(body))).collect()
}

/// Build a step's input under `directory` and return the path to import.
pub fn materialize(input: &Value, directory: &Path) -> PathBuf {
    match input["kind"].as_str().unwrap() {
        "directory" => {
            let path = directory.join("input");
            std::fs::create_dir_all(&path).unwrap();
            for (name, bytes) in files(input) {
                let target = path.join(&name);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, bytes).unwrap();
            }
            match input.get("open").and_then(Value::as_str) {
                Some(open) => path.join(open),
                None => path,
            }
        }
        "zip" => {
            use std::io::Write;
            let path = directory.join("input.zip");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            for (name, bytes) in files(input) {
                zip.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(&bytes).unwrap();
            }
            zip.finish().unwrap();
            path
        }
        other => panic!("unknown input kind {other}"),
    }
}

/// Replay one golden scenario and require the 2.1.0 results.
pub fn run_scenario(name: &str) {
    let golden = golden(&format!("{name}.json"));
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mut labels = Labels::default();
    for (index, step) in golden["steps"].as_array().unwrap().iter().enumerate() {
        let context = format!("{name} step {index}");
        let step_directory = directory.path().join(format!("step-{index}"));
        std::fs::create_dir(&step_directory).unwrap();
        if let Value::Object(delete) = &step["delete_before"] {
            let before = snapshot(&direct(store.path()), &mut labels);
            let prefix = delete["label_prefix"].as_str().unwrap();
            let target = before["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|record| record["label"].as_str().unwrap().starts_with(prefix))
                .unwrap_or_else(|| panic!("{context}: no record to delete"));
            let record_id: String = direct(store.path())
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
                        deleted_at: Some(delete["deleted_at"].as_str().unwrap().to_string()),
                    }],
                    ..Changes::default()
                })
                .unwrap();
        }
        let path = materialize(&step["input"], &step_directory);
        let options = ImportOptions { received_at: Some(step["received_at"].as_str().unwrap().to_string()) };
        let result = import::preview(&store, &path, &options);
        if let Some(expected) = step.get("preview_error").and_then(Value::as_str) {
            let message = match result {
                Ok(_) => panic!("{context}: the preview succeeded, but 2.1.0 refused it: {expected}"),
                // The golden messages come from 2.1.0 on Unix and use `/`.
                Err(error) => error
                    .to_string()
                    .replace(&step_directory.display().to_string(), "<dir>")
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            };
            assert_eq!(message, expected, "{context}");
            continue;
        }
        // 2.1.0 planned an invalid record and refused it at the write. The
        // native preview refuses it (deliberate change 9) with the same issues.
        if let Err(athleticstandard::Error::Invalid(error)) = &result {
            let refused = step["apply"]["error"]
                .as_str()
                .unwrap_or_else(|| panic!("{context}: the preview failed, but 2.1.0 wrote it: {error}"));
            let expected: Vec<String> = json::parse(refused)
                .unwrap_or_else(|_| panic!("{context}: 2.1.0 refused the write without issues: {refused}"))
                .as_array()
                .unwrap()
                .iter()
                .map(|issue| {
                    let path: Vec<String> = issue["path"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|part| match part {
                            Value::String(text) => text.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    format!("{}: {}", path.join("."), issue["message"].as_str().unwrap())
                })
                .collect();
            let actual: Vec<String> =
                error.issues.iter().map(|issue| format!("{}: {}", issue.path_text(), issue.message)).collect();
            assert_eq!(actual, expected, "{context}");
            assert_eq!(
                snapshot(&direct(store.path()), &mut labels)["database_revision"],
                step["database"]["database_revision"],
                "{context}"
            );
            continue;
        }
        let preview = result.unwrap_or_else(|error| panic!("{context}: {error}"));
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
        // Set ATH_GOLDEN_DUMP to a folder to write both previews for a diff tool.
        if let Ok(dump) = std::env::var("ATH_GOLDEN_DUMP") {
            std::fs::write(format!("{dump}/{name}-{index}-native.json"), json::to_deterministic_pretty(&output))
                .unwrap();
            std::fs::write(
                format!("{dump}/{name}-{index}-golden.json"),
                json::to_deterministic_pretty(&step["preview"]),
            )
            .unwrap();
        }
        assert_same_json(&output, &step["preview"], &format!("{context} preview"));
        let apply = match preview.apply(&mut store) {
            Ok(result) => serde_json::json!({ "result": json::to_value(&result) }),
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        };
        assert_same_json(&apply, &step["apply"], &format!("{context} apply"));
        let database = snapshot(&direct(store.path()), &mut labels);
        assert_same_json(&database, &step["database"], &format!("{context} database"));
    }
}
