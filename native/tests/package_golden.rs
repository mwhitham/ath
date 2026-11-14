//! Restore and re-export against the 2.1.0 golden packages.
//!
//! The golden files come from native/tools/reference/package-golden.ts, which
//! ran the 2.1.0 implementation on the same synthetic inputs. Each case
//! restores a package into an empty database, compares the preview, and
//! exports again. The export must equal the 2.1.0 export after only the
//! documented version change.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use athleticstandard::json::{self, Value};
use athleticstandard::model::time;
use athleticstandard::package::{apply_package_import, export_package, preview_package_import, ExportOptions};
use common::*;
use serde_json::json;

fn timed() -> ExportOptions {
    ExportOptions { exported_at: Some(EXPORTED_AT.into()) }
}

/// Restore `package` into a new empty database. Returns the database path
/// and the preview as JSON.
fn restore(package: &Path, directory: &Path) -> (std::path::PathBuf, Value) {
    let path = empty_destination(directory);
    let mut store = open(&path);
    let preview = preview_package_import(&store, package).unwrap();
    let preview_json = json::to_value(&preview);
    let applied = apply_package_import(&mut store, &preview, &[]).unwrap();
    assert!(applied.restored);
    assert_eq!(applied.database_revision, preview.source_database_revision);
    store.close().unwrap();
    (path, preview_json)
}

fn golden_json(path: &Path) -> Value {
    json::parse(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_demo_package_restores_and_exports_to_the_same_bytes() {
    let directory = temporary("ath-golden-demo-");
    let package = materialize_golden_package("demo", &directory);
    let (database, preview) = restore(&package, &directory);
    assert_eq!(preview, golden_json(&golden("demo").join("restore-preview.json")));
    let store = open_read_only(&database);
    let output = directory.join("exported");
    export_package(&store, &output, &timed()).unwrap();
    assert_same_package(&as_version_2_1_0(&files_below(&output)), &golden_package_files("demo"));
    assert_eq!(
        fs::read(directory.join("attachments").join(DEMO_ATTACHMENT)).unwrap(),
        fs::read(repository().join("examples/demo-athlete/attachments").join(DEMO_ATTACHMENT)).unwrap()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn the_version_2_0_0_package_restores_every_row_as_2_1_0_did() {
    let directory = temporary("ath-golden-200-");
    let package = repository().join("tests/fixtures/version-2-0-0/package");
    let (database, preview) = restore(&package, &directory);
    assert_eq!(preview, golden_json(&golden("version-2-0-0").join("restore-preview.json")));
    let expected_rows = fs::read_to_string(golden("version-2-0-0").join("database-rows.txt")).unwrap();
    let actual_rows = dump_rows(&database);
    for (index, (actual, expected)) in actual_rows.lines().zip(expected_rows.lines()).enumerate() {
        assert_eq!(actual, expected, "row line {index}");
    }
    assert_eq!(actual_rows, expected_rows);
    let store = open_read_only(&database);
    let output = directory.join("exported");
    export_package(&store, &output, &timed()).unwrap();
    assert_same_package(&as_version_2_1_0(&files_below(&output)), &golden_package_files("version-2-0-0"));
    fs::remove_dir_all(directory).unwrap();
}

// ---- The many-sample synthetic package -----------------------------------------------
//
// This builds the same input as `writeSyntheticPackage` in package-golden.ts.
// The input hashes are compared first, so a difference in the two builders is
// reported before any import runs.

const SYNTHETIC_DATABASE_ID: &str = "01890f3e-0000-7000-8000-000000000001";
const CREATED: &str = "2026-01-01T00:00:00.000Z";
const RECEIVED: &str = "2026-01-02T00:00:00.000Z";
const LATER: &str = "2026-01-03T00:00:00.000Z";
const BASE_MS: i64 = 1_767_225_600_000;
const HEART_COUNT: u64 = 50_006;
const SAME_TIME_FIRST: u64 = 49_997;
const SAME_TIME_LAST: u64 = 50_002;

fn rid(n: u64) -> String {
    format!("01890f3e-0000-7000-8000-{n:012x}")
}

fn at(ms: i64) -> String {
    time::format_utc_millis(ms)
}

fn sample_data(source: &str, quantity: &str, unit: &str, recorded_at: &str, value: f64, extra: Option<Value>) -> Value {
    let mut data = json!({
        "source_id": source,
        "received_at": RECEIVED,
        "received_at_basis": "import_time",
        "quantity": quantity,
        "recorded_at": recorded_at,
        "input_timestamp": {
            "timestamp_text": recorded_at,
            "timestamp_basis": "source_instant",
            "utc_offset_missing_reason": "not_provided_by_source",
        },
        "value": { "standard_value": value, "standard_unit": unit, "input_value": value, "input_unit": unit },
    });
    if let Some(Value::Object(extra)) = extra {
        data.as_object_mut().unwrap().extend(extra);
    }
    data
}

#[derive(Default)]
struct Built {
    records: Vec<Value>,
    revisions: Vec<Value>,
    deletions: Vec<Value>,
    import_keys: Vec<Value>,
}

impl Built {
    fn add(&mut self, record_id: &str, record_type: &str, datas: Vec<Value>, deleted: bool) {
        let count = datas.len();
        for (index, data) in datas.iter().enumerate() {
            self.revisions.push(json!({
                "record_id": record_id,
                "revision": index + 1,
                "previous_revision": if index == 0 { Value::Null } else { Value::from(index) },
                "became_known_at": if index == 0 { CREATED } else { LATER },
                "became_known_at_basis": "provider_import",
                "changed_by": { "author_type": "tool", "tool": "synthetic", "tool_version": "1.0.0" },
                "record_type": record_type,
                "data": data,
            }));
        }
        self.records.push(json!({
            "record_id": record_id,
            "created_at": CREATED,
            "current_revision": count,
            "is_deleted": deleted,
            "record_type": record_type,
            "data": datas.last().unwrap(),
        }));
        if deleted {
            self.deletions.push(json!({
                "record_id": record_id,
                "record_revision": count,
                "deleted_at": LATER,
                "deleted_by": { "author_type": "athlete" },
            }));
        }
    }

    fn key(&mut self, record_id: &str, method: &str, key: &str) {
        self.import_keys.push(
            json!({ "record_id": record_id, "match_key_method": method, "match_key": key, "imported_at": RECEIVED }),
        );
    }
}

fn special_supplied() -> Value {
    json!({
        "zeta": 1,
        "alpha": "β",
        "10": true,
        "9": null,
        "😀": "smile",
        "ﬁ": "ligature",
        "big": 9_007_199_254_740_991u64,
        "tiny": 1.5e-7,
        "huge": 1e21,
        "sum": 0.1 + 0.2,
        "negative": -2.5e-8,
        "control": "\u{0}\u{1f}\u{2028}\"\\/",
    })
}

fn heart_samples() -> Built {
    let source_a = rid(0x10);
    let source_b = rid(0x11);
    let mut built = Built::default();
    for i in 0..HEART_COUNT {
        let same_time = (SAME_TIME_FIRST..=SAME_TIME_LAST).contains(&i);
        let recorded_at = at(BASE_MS + (if same_time { SAME_TIME_FIRST } else { i }) as i64 * 1000);
        let record_id = if same_time { rid(0x200000 - i) } else { rid(0x100000 + i) };
        let extra = (i == 1).then(|| json!({ "supplied_fields": special_supplied() }));
        let current = sample_data(&source_a, "heart_rate", "bpm", &recorded_at, (40 + i % 150) as f64, extra);
        let datas = if i % 10_000 == 5_000 {
            let earlier = at(BASE_MS - 3_600_000 - i as i64);
            vec![sample_data(&source_b, "heart_rate", "bpm", &earlier, 30.0, None), current]
        } else {
            vec![current]
        };
        built.add(&record_id, "measurement_sample", datas, i == 7 || i == 50_003);
        if i % 20_000 == 1 {
            built.key(&record_id, "synthetic_v1", &format!("heart-{i}"));
        }
        if i == 1 {
            for key in ["z-key", "ä-key", "😀-key", "ﬁ-key"] {
                built.key(&record_id, "synthetic_text", key);
            }
        }
    }
    built
}

fn series_document(source: &str, quantity: &str, built: &Built) -> Value {
    let mut times: Vec<String> =
        built.records.iter().map(|record| record["data"]["recorded_at"].as_str().unwrap().to_string()).collect();
    times.sort();
    json!({
        "athleticstandard_version": "2.1.0",
        "source_id": source,
        "quantity": quantity,
        "start_at": times.first().unwrap(),
        "end_at": times.last().unwrap(),
        "records": built.records,
        "record_revisions": built.revisions,
        "record_deletions": built.deletions,
        "record_import_keys": built.import_keys,
    })
}

fn write_synthetic_package(directory: &Path) -> BTreeMap<String, String> {
    fs::create_dir_all(directory.join("series")).unwrap();
    let (source_a, source_b, source_c) = (rid(0x10), rid(0x11), rid(0x12));
    let mut respiratory = Built::default();
    for j in 0..3u64 {
        let data = sample_data(
            &source_a,
            "respiratory_rate",
            "brpm",
            &at(BASE_MS + j as i64 * 60_000),
            14.0 + j as f64 * 0.5,
            None,
        );
        respiratory.add(&rid(0x300000 + j), "measurement_sample", vec![data], false);
    }
    let mut source_b_samples = Built::default();
    for k in 0..2u64 {
        let data = sample_data(&source_b, "heart_rate", "bpm", &at(BASE_MS + k as i64 * 1000), (70 + k) as f64, None);
        source_b_samples.add(&rid(0x400000 + k), "measurement_sample", vec![data], false);
    }
    let groups = [
        (source_a.clone(), "heart_rate", heart_samples()),
        (source_a.clone(), "respiratory_rate", respiratory),
        (source_b.clone(), "heart_rate", source_b_samples),
    ];
    let mut hashes = BTreeMap::new();
    let mut references = Vec::new();
    for (index, (source, quantity, built)) in groups.iter().enumerate() {
        let file = format!("series/input-{}.ath-series.json", index + 1);
        let document = series_document(source, quantity, built);
        let bytes = json::to_deterministic_pretty(&document);
        fs::write(directory.join(&file), &bytes).unwrap();
        let hash = sha256(bytes.as_bytes());
        references.push(json!({
            "file": file,
            "sha256": hash,
            "source_id": source,
            "quantity": quantity,
            "start_at": document["start_at"],
            "end_at": document["end_at"],
            "sample_count": built.records.len(),
            "revision_count": built.revisions.len(),
            "deletion_count": built.deletions.len(),
            "import_key_count": built.import_keys.len(),
        }));
        hashes.insert(file, hash);
    }
    let mut main = Built::default();
    main.add(
        &source_a,
        "source",
        vec![json!({
            "source_type": "wearable",
            "data_writer": "Synthetic",
            "data_route": "synthetic_route",
            "vendor": "synthetic",
            "supplied_fields": { "b": 2, "a": 1, "1": "one" },
        })],
        false,
    );
    main.add(
        &source_b,
        "source",
        vec![
            json!({ "source_type": "wearable", "data_writer": "Synthetic", "data_route": "synthetic_route_b" }),
            json!({ "source_type": "wearable", "data_writer": "Synthetic", "data_route": "synthetic_route_b", "vendor": "synthetic" }),
        ],
        false,
    );
    main.add(
        &source_c,
        "source",
        vec![json!({ "source_type": "wearable", "data_writer": "Synthetic", "data_route": "synthetic_route_c" })],
        true,
    );
    main.key(&source_a, "synthetic_source", "A");
    let document = json!({
        "athleticstandard_version": "2.1.0",
        "database_id": SYNTHETIC_DATABASE_ID,
        "database_revision": 7,
        "records": main.records,
        "record_revisions": main.revisions,
        "record_deletions": main.deletions,
        "record_erasure_markers": [{ "record_id": rid(0x13), "erased_at": LATER, "reason_code": "athlete_request" }],
        "record_import_keys": main.import_keys,
        "measurement_series_files": references,
    });
    let bytes = json::to_deterministic_pretty(&document);
    fs::write(directory.join("athlete.ath.json"), &bytes).unwrap();
    hashes.insert("athlete.ath.json".into(), sha256(bytes.as_bytes()));
    hashes
}

#[test]
fn a_package_with_more_samples_than_one_series_file_round_trips_like_2_1_0() {
    let directory = temporary("ath-golden-many-");
    let input = directory.join("input");
    let hashes = write_synthetic_package(&input);
    let expected_hashes = golden_json(&golden("many-samples").join("input-hashes.json"));
    assert_eq!(json::to_value(&hashes), expected_hashes, "the synthetic input differs from the generator's");

    let (database, preview) = restore(&input, &directory);
    assert_eq!(preview, golden_json(&golden("many-samples").join("restore-preview.json")));
    let store = open_read_only(&database);
    let output = directory.join("exported");
    export_package(&store, &output, &timed()).unwrap();
    let exported = as_version_2_1_0(&files_below(&output));
    let expected_main = fs::read(golden("many-samples").join("main.json")).unwrap();
    let expected_manifest = fs::read(golden("many-samples").join("manifest.json")).unwrap();
    assert_eq!(String::from_utf8_lossy(&exported["athlete.ath.json"]), String::from_utf8_lossy(&expected_main));
    assert_eq!(String::from_utf8_lossy(&exported["export-manifest.json"]), String::from_utf8_lossy(&expected_manifest));
    // The manifest holds every file's hash and size; check each file against it.
    let manifest = json::parse(std::str::from_utf8(&expected_manifest).unwrap()).unwrap();
    let listed = manifest["files"].as_array().unwrap();
    assert_eq!(listed.len() + 1, exported.len(), "the manifest lists every file but itself");
    assert!(listed.iter().filter(|file| file["file"].as_str().unwrap().starts_with("series/")).count() > 3);
    for file in listed {
        let name = file["file"].as_str().unwrap();
        assert_eq!(sha256(&exported[name]), file["sha256"].as_str().unwrap(), "{name}");
        assert_eq!(exported[name].len() as u64, file["byte_count"].as_u64().unwrap(), "{name}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_unrepresentable_package_is_reported_like_2_1_0() {
    let directory = temporary("ath-golden-unrepresentable-");
    let package = directory.join("package");
    fs::create_dir(&package).unwrap();
    let mut document = golden_json(&golden("demo").join("main.json"));
    document["future_package_field"] = Value::String("not understood by v1".into());
    document["records"][0]["current_revision"] = Value::from(0);
    fs::write(package.join("athlete.ath.json"), format!("{}\n", json::to_string(&document))).unwrap();
    let destination = empty_destination(&directory);
    let store = open(&destination);
    let preview = preview_package_import(&store, &package).unwrap();
    let expected = golden_json(&golden("unrepresentable.json"));
    assert_eq!(json::to_value(&preview.unknown_fields), expected["unknown_fields"]);
    assert_eq!(json::to_value(&preview.unrepresentable_records), expected["unrepresentable_records"]);
    assert_eq!(Value::String(preview.source_database_id.clone()), expected["source_database_id"]);
    assert_eq!(Value::from(preview.source_database_revision), expected["source_database_revision"]);
    assert!(preview.restore);
    assert_eq!(preview.new_record_count, 0);
    let error = athleticstandard::package::load_package(&package).unwrap_err();
    assert_eq!(Value::String(error.to_string()), expected["message"]);
    fs::remove_dir_all(directory).unwrap();
}
