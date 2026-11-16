//! Run every validation vector recorded from the 2.1.0 validator through the
//! Rust validator. Valid and invalid verdicts, every issue's path, code, and
//! message, and the parsed output must agree.
//!
//! The vectors live in `tests/vectors/model/` and come from
//! `tools/reference/generate-model-vectors.ts`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use athleticstandard::json::{self, Value};
use athleticstandard::model::registry;

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/model")
}

/// Cases where 2.1.1 differs on purpose. Each entry names the reason.
fn deliberate_difference(schema: &str, case: &Value) -> Option<&'static str> {
    let name = case["name"].as_str().unwrap_or_default();
    let text = case.get("input_text").and_then(Value::as_str).unwrap_or_default();
    if text.contains("\\ud8") || text.contains("\\udc") || text.contains("\\uD8") || text.contains("\\uDC") {
        return Some("lone UTF-16 surrogates cannot be held in Rust text; the reader refuses them");
    }
    if text.contains("__proto__") || json::to_string(&case["input"]).contains("\"__proto__\"") {
        return Some("a __proto__ key is kept and checked like any other key instead of being dropped");
    }
    if schema == "RecordFormatVersion" || name.contains("2.1.1") || name.starts_with("athleticstandard_version:") {
        return Some("version 2.1.1 is accepted, so the version list in messages names it");
    }
    if text.contains("1e400") {
        return Some("a number too large for a double is refused when the JSON is read");
    }
    None
}

fn issue_text(issue: &Value) -> String {
    let path: Vec<String> = issue["path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| match segment {
            Value::String(text) => text.clone(),
            other => json::to_string(other),
        })
        .collect();
    format!("[{}] {} {}", path.join("."), issue["code"].as_str().unwrap(), issue["message"].as_str().unwrap())
}

#[test]
fn every_recorded_vector_matches() {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(vectors_dir()).unwrap().map(|entry| entry.unwrap().path()).collect();
    files.sort();
    assert!(files.len() >= 140, "expected the full vector set");
    let mut failures: Vec<String> = Vec::new();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut checked = 0usize;
    for file in files {
        let document = json::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let schema_name = document["schema"].as_str().unwrap().to_string();
        let shape = registry::shape(&schema_name).unwrap_or_else(|| panic!("no Rust shape for {schema_name}"));
        for case in document["cases"].as_array().unwrap() {
            if let Some(reason) = deliberate_difference(&schema_name, case) {
                *skipped.entry(reason).or_default() += 1;
                continue;
            }
            let name = case["name"].as_str().unwrap();
            let input = match case.get("input_text").and_then(Value::as_str) {
                Some(text) => match json::parse(text) {
                    Ok(value) => value,
                    Err(error) => {
                        failures.push(format!("{schema_name} / {name}: input text did not parse: {error}"));
                        continue;
                    }
                },
                None => case["input"].clone(),
            };
            checked += 1;
            let actual: Vec<String> = shape.issues(&input).iter().map(|issue| issue_text(&issue.to_json())).collect();
            let expected: Vec<String> = case["issues"].as_array().unwrap().iter().map(issue_text).collect();
            if actual != expected {
                failures.push(format!("{schema_name} / {name}:\n    expected {expected:?}\n    actual   {actual:?}"));
                continue;
            }
            if case["valid"].as_bool() == Some(true) {
                let expected_output = match case.get("output_text").and_then(Value::as_str) {
                    Some(text) => text.to_string(),
                    None => json::to_string(&case["output"]),
                };
                let actual_output = json::to_string(&shape.output(&input));
                if actual_output != expected_output {
                    failures.push(format!(
                        "{schema_name} / {name}: output differs\n    expected {expected_output}\n    actual   {actual_output}"
                    ));
                }
            }
        }
    }
    eprintln!("checked {checked} cases; deliberate differences skipped: {skipped:?}");
    if !failures.is_empty() {
        let shown: Vec<&String> = failures.iter().take(40).collect();
        panic!(
            "{} of {checked} cases differ:\n{}",
            failures.len(),
            shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
    }
}
