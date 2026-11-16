//! Every shipped skill file is checked, and every worked example in a
//! calculation skill is run (D246).
//!
//! A worked example is a fenced `ath-example` block of JSON:
//!
//! - `records`: short-form records (see `calculation_support`);
//! - `apply`: the short ID to run the skill's own calculation on, or
//!   `calculate`: a range request;
//! - `result`: the expected `result`, and optionally `missing`, `exclusions`,
//!   or `error`.
//!
//! Record and source IDs in the expected result are written by their short
//! names. The example runs the shipped file through the library, the same
//! path `ath apply` and `ath calculate` take.

mod calculation_support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use athleticstandard::assets::skill::{read_frontmatter, FrontmatterValue};
use athleticstandard::calculate::{CalculationFile, OneRecordCalculation};
use athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;
use calculation_support::{canon, Lab};

fn skills_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills")
}

fn skill_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(skills_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("SKILL.md").exists())
        .collect();
    dirs.sort();
    dirs
}

fn text_of(map: &std::collections::BTreeMap<String, FrontmatterValue>, key: &str) -> Option<String> {
    match map.get(key) {
        Some(FrontmatterValue::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn metadata(map: &std::collections::BTreeMap<String, FrontmatterValue>, key: &str) -> Option<String> {
    match map.get("metadata") {
        Some(FrontmatterValue::Map(nested)) => nested.get(key).cloned(),
        _ => None,
    }
}

/// The fenced blocks of one kind, in order.
fn blocks(text: &str, kind: &str) -> Vec<String> {
    let marker = format!("```{kind}\n");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&marker) {
        let body = &rest[start + marker.len()..];
        let end = body.find("\n```").expect("a closed block");
        out.push(body[..end].to_string());
        rest = &body[end + 4..];
    }
    out
}

/// Relative Markdown link targets, without anchors.
fn relative_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        let target = after[..end].split('#').next().unwrap_or("").to_string();
        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            out.push(target);
        }
        rest = &after[end..];
    }
    out
}

#[test]
fn every_skill_file_has_a_valid_shape() {
    let mut names = BTreeSet::new();
    for dir in skill_dirs() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        let front = read_frontmatter(&text);
        assert_eq!(text_of(&front, "name").as_deref(), Some(name.as_str()), "{name}: name matches its folder");
        assert!(name.len() <= 64 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        let description = text_of(&front, "description").unwrap_or_default();
        assert!(description.len() >= 80, "{name}: a description that says when to use the skill");
        assert!(description.len() <= 1024, "{name}: description is {} characters", description.len());
        assert_eq!(text_of(&front, "license").as_deref(), Some("MIT"), "{name}: license");
        assert_eq!(metadata(&front, "version").as_deref(), Some(ATHLETIC_STANDARD_VERSION), "{name}: version");
        let kind = metadata(&front, "kind").unwrap_or_default();
        assert!(["tool", "calculation", "coaching"].contains(&kind.as_str()), "{name}: kind {kind}");
        assert_eq!(kind == "tool", name == "athletic-standard", "{name}: only the tool skill is kind tool");
        let compatibility = text_of(&front, "compatibility").unwrap_or_default();
        assert!(
            compatibility.contains(ATHLETIC_STANDARD_VERSION),
            "{name}: compatibility names {ATHLETIC_STANDARD_VERSION}"
        );
        for link in relative_links(&text) {
            assert!(dir.join(&link).exists(), "{name}: link {link} does not resolve");
        }
        if kind == "calculation" {
            let parsed = CalculationFile::parse(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(parsed.name, name, "{name}: the block name is the skill name");
            assert!(parsed.calculation.method_notice().is_none(), "{name}: uses a replaced operation");
            assert!(
                !matches!(parsed.calculation, OneRecordCalculation::Unknown(_)),
                "{name}: an operation this program has"
            );
            assert!(
                compatibility.contains(&format!("ath {} {name}", command_of(&parsed))),
                "{name}: compatibility command"
            );
            assert!(text.contains("## Worked example"), "{name}: a worked example");
            assert!(!blocks(&text, "json ath-example").is_empty(), "{name}: an ath-example block");
            for heading in ["## Method", "## Reading the result", "## Limits"] {
                assert!(text.contains(heading), "{name}: {heading}");
            }
        } else {
            assert!(blocks(&text, "ath-calculation").is_empty(), "{name}: no calculation block");
        }
        names.insert(name);
    }
    assert!(names.contains("athletic-standard"));
    assert!(names.iter().filter(|name| name.as_str() != "athletic-standard").count() >= 24);
}

fn command_of(parsed: &CalculationFile) -> &'static str {
    if matches!(parsed.calculation, OneRecordCalculation::Range(_)) {
        "calculate"
    } else {
        "apply"
    }
}

fn run_example(skill: &str, example: &serde_json::Value) -> Result<(), String> {
    let mut lab = Lab::new();
    lab.write(example["records"].as_array().ok_or("records")?);
    let output = if let Some(target) = example.get("apply").and_then(|value| value.as_str()) {
        lab.apply_shipped(skill, target)
    } else {
        lab.calculate(&example["calculate"])
    };
    let output = lab.shorten(&output);
    for key in ["result", "missing", "exclusions", "error"] {
        if let Some(expected) = example.get(key) {
            if canon(&output[key]) != canon(expected) {
                return Err(format!(
                    "{skill}: {key} differs\n  expected {}\n  actual   {}\n  output {output:#}",
                    expected, output[key]
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn every_worked_example_gives_the_printed_result() {
    let mut failures = Vec::new();
    let mut ran = 0;
    for dir in skill_dirs() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        for (index, block) in blocks(&text, "json ath-example").iter().enumerate() {
            let example: serde_json::Value =
                serde_json::from_str(block).unwrap_or_else(|error| panic!("{name} example {index}: {error}"));
            ran += 1;
            if let Err(message) = run_example(&name, &example) {
                failures.push(format!("example {index} of {message}"));
            }
        }
    }
    assert!(failures.is_empty(), "{} examples differ:\n{}", failures.len(), failures.join("\n\n"));
    assert!(ran >= 24, "ran {ran} examples");
}

/// Every Markdown file of a coaching skill: `SKILL.md` and `references/*.md`.
fn coaching_files() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for dir in skill_dirs() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        if metadata(&read_frontmatter(&text), "kind").as_deref() != Some("coaching") {
            continue;
        }
        out.push((name.clone(), dir.join("SKILL.md")));
        if let Ok(entries) = std::fs::read_dir(dir.join("references")) {
            let mut files: Vec<PathBuf> = entries.map(|entry| entry.unwrap().path()).collect();
            files.sort();
            for file in files.into_iter().filter(|file| file.extension().is_some_and(|ext| ext == "md")) {
                out.push((name.clone(), file));
            }
        }
    }
    out
}

/// Every `ath-record-example` block in a coaching skill is an array of records
/// that the model accepts, with extensions that pass the record extension rules.
#[test]
fn every_coaching_record_example_is_a_valid_record() {
    use athleticstandard::json;
    use athleticstandard::model::{registry, RecordData, RecordType};
    let mut checked = 0;
    let mut failures = Vec::new();
    for (_skill, file) in coaching_files() {
        let text = std::fs::read_to_string(&file).unwrap();
        let shown = file.strip_prefix(skills_root()).unwrap().display().to_string();
        for (index, block) in blocks(&text, "json ath-record-example").iter().enumerate() {
            let items: Vec<serde_json::Value> = match serde_json::from_str(block) {
                Ok(serde_json::Value::Array(items)) => items,
                Ok(_) => {
                    failures.push(format!("{shown} block {index}: not a JSON array"));
                    continue;
                }
                Err(error) => {
                    failures.push(format!("{shown} block {index}: {error}"));
                    continue;
                }
            };
            for (position, item) in items.iter().enumerate() {
                checked += 1;
                let place = format!("{shown} block {index} item {position}");
                let Some(kind) = item["record_type"].as_str().and_then(RecordType::parse) else {
                    failures.push(format!("{place}: unknown record_type {}", item["record_type"]));
                    continue;
                };
                for key in item.as_object().unwrap().keys() {
                    if !["record_type", "record_id", "data", "extensions"].contains(&key.as_str()) {
                        failures.push(format!("{place}: unexpected key {key}"));
                    }
                }
                let data = json::parse(&item["data"].to_string()).unwrap();
                if let Err(error) = RecordData::decode(kind, &data) {
                    failures.push(format!("{place} ({kind:?}): {error:?}"));
                }
                if let Some(extensions) = item.get("extensions") {
                    let value = json::parse(&extensions.to_string()).unwrap();
                    let issues = registry::shape("RecordExtensions").unwrap().issues(&value);
                    if !issues.is_empty() {
                        failures.push(format!("{place}: extensions {issues:?}"));
                    }
                }
                if let Some(id) = item.get("record_id").and_then(|value| value.as_str()) {
                    if !athleticstandard::model::is_record_id(id) {
                        failures.push(format!("{place}: record_id {id} is not a UUIDv7"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} invalid record examples:\n{}", failures.len(), failures.join("\n"));
    if !coaching_files().is_empty() {
        assert!(checked >= 10, "only {checked} record examples in the coaching skills");
    }
}

#[test]
fn the_examples_folder_is_not_needed() {
    // Examples live inside each SKILL.md so an installed copy carries them.
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/examples").exists());
}

/// `skills.sh.json` puts every shipped skill in exactly one group and names no
/// skill that does not exist (D248).
#[test]
fn the_skills_sh_groups_list_every_skill_once() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("skills.sh.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut grouped: Vec<String> = Vec::new();
    for group in config["groupings"].as_array().unwrap() {
        assert!(group["title"].as_str().is_some_and(|title| !title.is_empty()));
        for skill in group["skills"].as_array().unwrap() {
            grouped.push(skill.as_str().unwrap().to_string());
        }
    }
    let mut sorted = grouped.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), grouped.len(), "a skill is listed twice");
    let shipped: Vec<String> =
        skill_dirs().iter().map(|dir| dir.file_name().unwrap().to_string_lossy().to_string()).collect();
    let mut shipped_sorted = shipped.clone();
    shipped_sorted.sort();
    assert_eq!(sorted, shipped_sorted);
}
