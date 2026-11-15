//! Real health data never enters Git (AGENTS.md).
//!
//! A tracked file that looks like health data is allowed only when its exact
//! synthetic contents were reviewed. The reviewed paths and SHA-256 hashes are
//! in `tools/public-data-allowlist.json`. A changed hash is a prompt for another
//! review, not proof of origin.
//!
//! The test also requires the private-data rules in `.gitignore`, and an
//! explicit `!path` exception there for every allowed health-data file.
//!
//! It reads the file list from `git ls-files`, so it needs Git and a checkout.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// Rules `.gitignore` must contain, so a real export is not added by accident.
const REQUIRED_IGNORE_RULES: &[&str] = &[
    ".private-data/",
    "private-validation/",
    "validation-data/",
    "*.ath.json",
    "*.ath.sqlite",
    "*.ath.sqlite-wal",
    "*.ath.sqlite-shm",
    "*.fit",
    "*.gpx",
    "*.xml",
    "*.zip",
    "*.csv",
];

/// Endings that always mean health data, outside `schema/`.
const SENSITIVE_ENDINGS: &[&str] = &[".ath.json", ".ath.sqlite", ".csv", ".fit", ".gpx", ".tcx", ".xml", ".zip"];

/// Words that make a `.json` file name look like a provider or athlete export.
const SENSITIVE_JSON_WORDS: &[&str] = &["athlete", "export", "garmin", "healthkit", "oura", "whoop"];

/// The folder that holds `.gitignore`: the repository root.
fn repository_root() -> PathBuf {
    let mut directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !directory.join(".gitignore").is_file() {
        assert!(directory.pop(), "no .gitignore above {}", env!("CARGO_MANIFEST_DIR"));
    }
    directory
}

fn tracked_files(root: &Path) -> BTreeSet<String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("the public-data check needs git: {error}"));
    assert!(output.status.success(), "git ls-files failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout)
        .expect("tracked paths are UTF-8")
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect()
}

/// `apple-health`, `apple_health`, `apple health`, or `applehealth`.
fn names_apple_health(name: &str) -> bool {
    name.match_indices("apple").any(|(index, _)| {
        let rest = &name[index + "apple".len()..];
        let rest = rest.strip_prefix(['-', '_', ' ']).unwrap_or(rest);
        rest.starts_with("health")
    })
}

fn is_health_data_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    if lower.starts_with("schema/") {
        return false;
    }
    if SENSITIVE_ENDINGS.iter().any(|ending| lower.ends_with(ending)) {
        return true;
    }
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    name.ends_with(".json") && (names_apple_health(name) || SENSITIVE_JSON_WORDS.iter().any(|word| name.contains(word)))
}

fn is_fixture_path(path: &str) -> bool {
    path.starts_with("examples/demo-athlete/") || path.starts_with("tests/fixtures/exports/")
}

/// The reviewed list, in file order.
fn allowlist(root: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(root.join("tools/public-data-allowlist.json")).unwrap();
    let document: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut approved = BTreeMap::new();
    for entry in document["fixtures"].as_array().expect("fixtures is an array") {
        let path = entry["path"].as_str().expect("path").to_string();
        let sha256 = entry["sha256"].as_str().expect("sha256").to_string();
        assert!(approved.insert(path.clone(), sha256).is_none(), "{path} is listed twice");
    }
    approved
}

fn sha256(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn only_reviewed_synthetic_health_data_is_tracked() {
    let root = repository_root();
    let tracked = tracked_files(&root);
    let approved = allowlist(&root);
    let ignore_lines: BTreeSet<String> = std::fs::read_to_string(root.join(".gitignore"))
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();

    let mut errors = Vec::new();
    for path in &tracked {
        if (is_health_data_file(path) || is_fixture_path(path)) && !approved.contains_key(path) {
            errors.push(format!(
                "{path}: health-data file is not on the synthetic-fixture allowlist. Remove it from Git. \
                 If it is invented test data, review it and add its exact path and hash to \
                 tools/public-data-allowlist.json."
            ));
        }
    }
    for (path, expected) in &approved {
        if !tracked.contains(path) {
            errors.push(format!("{path}: approved synthetic fixture is missing or is not tracked."));
            continue;
        }
        let actual = sha256(&root.join(path));
        if &actual != expected {
            errors.push(format!(
                "{path}: approved synthetic fixture changed (expected {expected}, got {actual}). \
                 Confirm every value is invented, then update the reviewed hash."
            ));
        }
        if is_health_data_file(path) && !ignore_lines.contains(&format!("!{path}")) {
            errors.push(format!("{path}: add an explicit !{path} exception to .gitignore."));
        }
    }
    for rule in REQUIRED_IGNORE_RULES {
        if !ignore_lines.contains(*rule) {
            errors.push(format!(".gitignore: required private-data rule is missing: {rule}"));
        }
    }
    assert!(
        errors.is_empty(),
        "Public-data check failed:\n- {}\nReal health exports belong in encrypted storage outside Git.",
        errors.join("\n- ")
    );
}

#[test]
fn health_data_names_are_recognized() {
    for path in [
        "a/export.xml",
        "a/b.CSV",
        "x.fit",
        "x.gpx",
        "x.tcx",
        "x.zip",
        "athlete.ath.json",
        "x.ath.sqlite",
        "a/my-apple-health.json",
        "apple_health.json",
        "apple health.json",
        "applehealth.json",
        "Whoop-cycles.json",
        "garmin.json",
        "healthkit.json",
        "oura.json",
        "athlete-profile.json",
        "export-manifest.json",
    ] {
        assert!(is_health_data_file(path), "{path}");
    }
    for path in [
        "schema/2.1.1/export-manifest.schema.json",
        "schema/x.csv",
        "apple.json",
        "health.json",
        "notes.md",
        "package.json",
        "a/whoop.md",
    ] {
        assert!(!is_health_data_file(path), "{path}");
    }
}
