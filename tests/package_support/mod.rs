//! Helpers shared by the package tests.

#![allow(dead_code)]

#[path = "../temporary_root.rs"]
mod temporary_root;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::files;
use athleticstandard::store::{self, OpenOptions, Store};

pub const DEMO_ATTACHMENT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000e.png";
pub const EXPORTED_AT: &str = "2026-11-14T00:00:00.000Z";
pub const DESTINATION_ID: &str = "01890f3e-0000-7000-8000-0000000000ff";
pub const DESTINATION_CREATED_AT: &str = "2026-11-14T00:00:00.000Z";

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn golden(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/package").join(case)
}

/// Remove a test folder. Windows cannot remove a folder while the test still
/// holds its database open; the test program's root is emptied on its next
/// run, so the folder is left for then.
pub fn remove_dir(path: &Path) {
    if let Err(error) = fs::remove_dir_all(path) {
        if !cfg!(windows) {
            panic!("could not remove {}: {error}", path.display());
        }
    }
}

pub fn temporary(prefix: &str) -> PathBuf {
    temporary_root::new_dir(prefix)
}

/// Copy the demo database and its attachment into `directory`, then apply
/// migration step 5 so this library can open it.
pub fn migrated_demo_copy(directory: &Path) -> PathBuf {
    let demo = repository().join("examples/demo-athlete");
    let path = directory.join("athlete.ath.sqlite");
    fs::copy(demo.join("athlete.ath.sqlite"), &path).unwrap();
    files::create_private_dir(&directory.join("attachments")).unwrap();
    fs::copy(demo.join("attachments").join(DEMO_ATTACHMENT), directory.join("attachments").join(DEMO_ATTACHMENT))
        .unwrap();
    files::set_owner_only(&path).unwrap();
    migrate_to_current(&path);
    path
}

/// Apply every missing migration step without a backup. Test fixtures only.
pub fn migrate_to_current(path: &Path) {
    let mut opened = Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let from = opened.metadata().sql_schema_version;
    opened
        .transaction(|tx| {
            for version in from + 1..=store::SQL_SCHEMA_VERSION {
                store::migrate::apply_step(tx, version)?;
            }
            Ok(())
        })
        .unwrap();
    opened.close().unwrap();
}

pub fn open(path: &Path) -> Store {
    Store::open(path, OpenOptions::default()).unwrap()
}

pub fn open_read_only(path: &Path) -> Store {
    Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap()
}

pub fn empty_destination(directory: &Path) -> PathBuf {
    let path = directory.join("destination.ath.sqlite");
    Store::create(
        &path,
        store::CreateOptions {
            database_id: Some(DESTINATION_ID.into()),
            created_at: Some(DESTINATION_CREATED_AT.into()),
        },
    )
    .unwrap()
    .close()
    .unwrap();
    path
}

/// Every file below `directory`, by relative path with `/` separators.
pub fn files_below(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(directory, directory, &mut out);
    out
}

pub fn sha256(bytes: &[u8]) -> String {
    files::durable::sha256_hex(bytes)
}

const NEW_VERSION_LINE: &str = "\n  \"athleticstandard_version\": \"2.1.1\",\n";
const OLD_VERSION_LINE: &str = "\n  \"athleticstandard_version\": \"2.1.0\",\n";

fn replace_version_line(text: &str, name: &str) -> String {
    assert_eq!(text.matches(NEW_VERSION_LINE).count(), 1, "{name} must name version 2.1.1 once at the top level");
    assert!(text.starts_with(&format!("{{{NEW_VERSION_LINE}")), "{name} must start with its version");
    text.replacen(NEW_VERSION_LINE, OLD_VERSION_LINE, 1)
}

/// Undo the one documented 2.1.1 difference in a package: the top-level
/// version of each JSON document, and the hashes that change with it. Byte
/// counts do not change, because both versions have the same length.
pub fn as_version_2_1_0(package: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut hashes: Vec<(String, String)> = Vec::new();
    for (name, bytes) in package {
        if name.starts_with("series/") {
            let text = replace_version_line(std::str::from_utf8(bytes).unwrap(), name);
            hashes.push((sha256(bytes), sha256(text.as_bytes())));
            out.insert(name.clone(), text.into_bytes());
        }
    }
    let swap = |mut text: String, hashes: &[(String, String)]| {
        for (new, old) in hashes {
            text = text.replace(new.as_str(), old.as_str());
        }
        text
    };
    let main = package.get("athlete.ath.json").expect("main document");
    let main_text = swap(replace_version_line(std::str::from_utf8(main).unwrap(), "athlete.ath.json"), &hashes);
    hashes.push((sha256(main), sha256(main_text.as_bytes())));
    out.insert("athlete.ath.json".into(), main_text.into_bytes());
    let manifest = package.get("export-manifest.json").expect("manifest");
    let manifest_text =
        swap(replace_version_line(std::str::from_utf8(manifest).unwrap(), "export-manifest.json"), &hashes);
    out.insert("export-manifest.json".into(), manifest_text.into_bytes());
    for (name, bytes) in package {
        out.entry(name.clone()).or_insert_with(|| bytes.clone());
    }
    out
}

/// A golden case as a package folder: main.json and manifest.json under their
/// package names, its series files, and the demo attachment when listed.
pub fn materialize_golden_package(case: &str, directory: &Path) -> PathBuf {
    let source = golden(case);
    let target = directory.join(format!("{case}-package"));
    fs::create_dir_all(&target).unwrap();
    fs::copy(source.join("main.json"), target.join("athlete.ath.json")).unwrap();
    fs::copy(source.join("manifest.json"), target.join("export-manifest.json")).unwrap();
    if source.join("series").is_dir() {
        fs::create_dir_all(target.join("series")).unwrap();
        for entry in fs::read_dir(source.join("series")).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), target.join("series").join(entry.file_name())).unwrap();
        }
    }
    let manifest = fs::read_to_string(source.join("manifest.json")).unwrap();
    if manifest.contains(DEMO_ATTACHMENT) {
        fs::create_dir_all(target.join("attachments")).unwrap();
        fs::copy(
            repository().join("examples/demo-athlete/attachments").join(DEMO_ATTACHMENT),
            target.join("attachments").join(DEMO_ATTACHMENT),
        )
        .unwrap();
    }
    target
}

/// The golden package files of a case, under their package names.
pub fn golden_package_files(case: &str) -> BTreeMap<String, Vec<u8>> {
    let directory = temporary("ath-golden-files-");
    let package = materialize_golden_package(case, &directory);
    let files = files_below(&package);
    fs::remove_dir_all(directory).unwrap();
    files
}

/// Compare two packages file by file, naming the first difference.
pub fn assert_same_package(actual: &BTreeMap<String, Vec<u8>>, expected: &BTreeMap<String, Vec<u8>>) {
    let actual_names: Vec<&String> = actual.keys().collect();
    let expected_names: Vec<&String> = expected.keys().collect();
    assert_eq!(actual_names, expected_names, "package file lists differ");
    for (name, bytes) in expected {
        if actual[name] != *bytes {
            let actual_text = String::from_utf8_lossy(&actual[name]);
            let expected_text = String::from_utf8_lossy(bytes);
            let line = actual_text.lines().zip(expected_text.lines()).position(|(a, b)| a != b);
            panic!(
                "{name} differs at line {line:?}:\n actual: {:?}\n expect: {:?}",
                line.and_then(|line| actual_text.lines().nth(line)),
                line.and_then(|line| expected_text.lines().nth(line)),
            );
        }
    }
}

pub const TABLES: &[&str] = &[
    "records",
    "record_revisions",
    "record_deletions",
    "record_erasure_markers",
    "record_import_keys",
    "athlete_profile",
    "sources",
    "source_devices",
    "point_measurements",
    "point_measurement_calculations",
    "point_measurement_calculation_inputs",
    "measurement_samples",
    "sleep_sessions",
    "workout_sessions",
    "workout_session_segments",
    "workout_route_points",
    "benchmark_results",
    "benchmark_result_segments",
    "provider_scores",
    "provider_summaries",
    "provider_reporting_periods",
    "athlete_reports",
    "nutrition_entries",
    "reported_nutrition_items",
    "attachments",
    "record_attachment_links",
    "benchmark_definitions",
    "predictions",
    "prediction_result_links",
    "prediction_grades",
    "prediction_error_analyses",
    "prediction_error_causes",
    "training_plans",
    "planned_sessions",
    "planned_session_blocks",
    "coaching_notes",
    "saved_training_reviews",
    "goals",
    "nutrition_model_interpretations",
    "authored_record_evidence",
];

/// Every athlete-table row as a JSON array, sorted, as the generator dumps them.
pub fn dump_rows(path: &Path) -> String {
    let connection = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut out = String::new();
    for table in TABLES {
        out.push_str(&format!("## {table}\n"));
        let mut statement = connection.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let columns = statement.column_count();
        let mut rows: Vec<String> = statement
            .query_map([], |row| {
                let mut values = Vec::new();
                for index in 0..columns {
                    values.push(match row.get::<_, rusqlite::types::Value>(index)? {
                        rusqlite::types::Value::Null => athleticstandard::json::Value::Null,
                        rusqlite::types::Value::Integer(number) => athleticstandard::json::Value::from(number),
                        rusqlite::types::Value::Real(number) => athleticstandard::json::Value::from(number),
                        rusqlite::types::Value::Text(text) => athleticstandard::json::Value::String(text),
                        rusqlite::types::Value::Blob(_) => {
                            panic!("athlete tables hold no blobs")
                        }
                    });
                }
                Ok(athleticstandard::json::to_string(&athleticstandard::json::Value::Array(values)))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.sort_by(|left, right| athleticstandard::json::compare_utf16(left, right));
        for row in rows {
            out.push_str(&row);
            out.push('\n');
        }
    }
    out
}
