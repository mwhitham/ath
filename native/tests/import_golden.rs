//! The native provider import compared with 2.1.0.
//!
//! Each golden file was written by `native/tools/reference/import-golden.ts`
//! from the 2.1.0 library. A scenario replays the same steps here and requires
//! the same preview, the same write result or refusal, and the same database:
//! every record's stored JSON for every revision, revision metadata, and import
//! keys. Record IDs are compared through labels built from match keys. The only
//! other normalization is the tool version in `changed_by`, which names the
//! program version that wrote the revision.
#![cfg_attr(not(unix), allow(unused_imports, dead_code))]

mod import_support;

use athleticstandard::import::{self, collate, instant, ImportOptions};
use athleticstandard::json::{self, Value};
use athleticstandard::model::RecordActor;
use athleticstandard::store::{Changes, Deletion};
use import_support::{assert_same_json, golden, materialize, new_store, snapshot, KeyFact, Labels, TempDir};

fn run_scenario(name: &str) {
    let golden = golden(&format!("{name}.json"));
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mut labels = Labels::default();
    for (index, step) in golden["steps"].as_array().unwrap().iter().enumerate() {
        let context = format!("{name} step {index}");
        let step_directory = directory.path().join(format!("step-{index}"));
        std::fs::create_dir(&step_directory).unwrap();
        if let Value::Object(delete) = &step["delete_before"] {
            let before = snapshot(store.connection(), &mut labels);
            let prefix = delete["label_prefix"].as_str().unwrap();
            let target = before["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|record| record["label"].as_str().unwrap().starts_with(prefix))
                .unwrap_or_else(|| panic!("{context}: no record to delete"));
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
                        deleted_at: Some(delete["deleted_at"].as_str().unwrap().to_string()),
                    }],
                    ..Changes::default()
                })
                .unwrap();
        }
        let path = materialize(&step["input"], &step_directory);
        let options = ImportOptions { received_at: Some(step["received_at"].as_str().unwrap().to_string()) };
        let preview = import::preview(&store, &path, &options).unwrap_or_else(|error| panic!("{context}: {error}"));
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

#[test]
fn fixture_fresh_repeat_and_correction_match_2_1_0() {
    run_scenario("cycles-fixture");
}

#[test]
fn duplicate_rows_and_ambiguous_correction_match_2_1_0() {
    run_scenario("cycles-duplicate");
}

#[test]
fn edge_rows_match_2_1_0() {
    run_scenario("cycles-edges");
}

#[test]
fn folder_zip_and_single_file_containers_match_2_1_0() {
    run_scenario("cycles-containers");
}

#[test]
fn a_deleted_record_named_by_a_later_import_is_a_conflict_as_in_2_1_0() {
    run_scenario("cycles-deleted");
}

#[test]
fn skip_reasons_sort_as_2_1_0_sorted_them() {
    let golden = golden("collation.json");
    let mut input: Vec<String> =
        golden["input"].as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect();
    input.sort_by(|left, right| collate::locale_compare(left, right));
    let expected: Vec<String> =
        golden["sorted"].as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect();
    assert_eq!(input, expected);
}

#[test]
fn provider_instants_read_as_2_1_0_read_them() {
    for case in golden("instants.json")["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let expected = case["utc"].as_str();
        assert_eq!(instant::normalized_utc(text).ok().as_deref(), expected, "{text}");
    }
}

#[cfg(unix)]
fn symlink(target: impl AsRef<std::path::Path>, link: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_: impl AsRef<std::path::Path>, _: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    unreachable!("link cases run only on Unix")
}

#[test]
fn detection_refusals_match_2_1_0() {
    for case in golden("detection.json")["cases"].as_array().unwrap() {
        // Creating a symbolic link needs Unix or extra Windows privileges.
        let links = ["symbolic_link", "linked_csv_in_directory"];
        if cfg!(not(unix)) && links.contains(&case["name"].as_str().unwrap()) {
            continue;
        }
        let directory = TempDir::new();
        let root = directory.path();
        let path = match case["name"].as_str().unwrap() {
            "missing" => root.join("missing"),
            "empty_directory" => {
                std::fs::create_dir(root.join("input")).unwrap();
                root.join("input")
            }
            "privacy_portal_directory" => {
                std::fs::create_dir_all(root.join("input/health")).unwrap();
                root.join("input")
            }
            "unknown_csv" => {
                std::fs::write(root.join("table.csv"), "a,b\n1,2\n").unwrap();
                root.join("table.csv")
            }
            "unsupported_file" => {
                std::fs::write(root.join("notes.txt"), "x").unwrap();
                root.join("notes.txt")
            }
            "unknown_zip" => {
                import_support::write_zip(&root.join("data.zip"), &[("a.csv".into(), "a,b\n".into())]);
                root.join("data.zip")
            }
            "privacy_portal_zip" => {
                import_support::write_zip(&root.join("data.zip"), &[("health/sleeps.csv".into(), "a\n".into())]);
                root.join("data.zip")
            }
            "symbolic_link" => {
                import_support::write_directory(&root.join("real"), &import_support::fixture_files());
                symlink(root.join("real"), root.join("link")).unwrap();
                root.join("link")
            }
            "linked_csv_in_directory" => {
                std::fs::create_dir(root.join("input")).unwrap();
                std::fs::write(root.join("outside.csv"), "a\n").unwrap();
                symlink(root.join("outside.csv"), root.join("input/a.csv")).unwrap();
                root.join("input")
            }
            other => panic!("unknown detection case {other}"),
        };
        let message = match import::detect_export(&path) {
            Ok(_) => "detected".to_string(),
            Err(error) => error.to_string().replace(&root.display().to_string(), "<dir>"),
        };
        assert_eq!(message, case["message"].as_str().unwrap(), "{}", case["name"]);
    }
}
