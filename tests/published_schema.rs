//! The generated JSON Schema must match the published 2.1.1 files except for
//! the version list, the schema IDs, and the optional 2.1.2 additions (D238,
//! D239). The published 2.1.1 files must still match the published 2.1.0 files
//! except for their version markers, because 2.1.1 changed no record field.

use athleticstandard::json::{self, Value};
use athleticstandard::model::{json_schema, package};

fn published(version: &str, name: &str) -> Value {
    let path = format!("{}/schema/{version}/{name}", env!("CARGO_MANIFEST_DIR"));
    json::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Drop `newer` from version lists and rewrite its schema IDs to `older`.
fn rewrite_versions(value: &mut Value, newer: &str, older: &str) {
    match value {
        Value::Array(items) => {
            let versions: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            if versions.first() == Some(&"1.0.0") && versions.last() == Some(&newer) {
                items.pop();
            }
            items.iter_mut().for_each(|item| rewrite_versions(item, newer, older));
        }
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get_mut("$id") {
                *id = id.replace(&format!("/{newer}/"), &format!("/{older}/"));
            }
            map.values_mut().for_each(|item| rewrite_versions(item, newer, older));
        }
        _ => {}
    }
}

/// Remove exactly the optional fields and the enum value that 2.1.2 adds:
/// `session_effort`, `athlete_report.workout_session_id`, and the strength-set
/// `movement`, `load_basis`, and `implement_count`. Returns how many it removed.
fn remove_2_1_2_additions(value: &mut Value) -> usize {
    let mut removed = 0;
    match value {
        Value::Array(items) => {
            let report_types = items.iter().any(|item| item.as_str() == Some("sleep_quality"));
            if report_types {
                let before = items.len();
                items.retain(|item| item.as_str() != Some("session_effort"));
                removed += before - items.len();
            }
            for item in items.iter_mut() {
                removed += remove_2_1_2_additions(item);
            }
        }
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get_mut("properties") {
                let strength_set = ["reps", "weight", "weight_unit"].iter().all(|key| properties.contains_key(*key));
                if strength_set {
                    for key in ["movement", "load_basis", "implement_count"] {
                        removed += usize::from(properties.remove(key).is_some());
                    }
                }
                if properties.contains_key("report_type") && properties.contains_key("reported_via") {
                    removed += usize::from(properties.remove("workout_session_id").is_some());
                }
            }
            for item in map.values_mut() {
                removed += remove_2_1_2_additions(item);
            }
        }
        _ => {}
    }
    removed
}

fn first_difference(expected: &str, actual: &str, name: &str) {
    if expected != actual {
        let out = std::env::temp_dir().join(format!("generated-{name}"));
        std::fs::write(&out, actual).unwrap();
        let first = expected.lines().zip(actual.lines()).position(|(a, b)| a != b);
        panic!("{name} differs from the published schema at line {first:?}; wrote {}", out.display());
    }
}

/// Compare a generated document with the published 2.1.1 file. `additions` is
/// how many 2.1.2 additions the document must contain.
fn compare(generated: Value, name: &str, additions: usize) {
    let mut generated = generated;
    rewrite_versions(&mut generated, package::ATHLETIC_STANDARD_VERSION, "2.1.1");
    assert_eq!(remove_2_1_2_additions(&mut generated), additions, "{name}: the 2.1.2 additions");
    first_difference(&json::to_pretty(&published("2.1.1", name)), &json::to_pretty(&generated), name);
}

#[test]
fn package_schema_matches_published_layout() {
    let id = format!(
        "https://athleticstandard.ai/schema/{}/athleticstandard.schema.json",
        package::ATHLETIC_STANDARD_VERSION
    );
    // Each record shape appears twice in the package schema, once for current
    // records and once for retained revisions. Each copy has the three
    // strength-set fields, the report type, and the report's workout link.
    let additions = 2 * (3 + 1 + 1);
    compare(
        json_schema::document(package::package_schema(), &id, "Athletic Standard portable package document"),
        "athleticstandard.schema.json",
        additions,
    );
}

#[test]
fn series_schema_matches_published_layout() {
    let id = format!(
        "https://athleticstandard.ai/schema/{}/measurement-series.schema.json",
        package::ATHLETIC_STANDARD_VERSION
    );
    compare(
        json_schema::document(
            package::measurement_series_file_schema(),
            &id,
            "Athletic Standard measurement-series file",
        ),
        "measurement-series.schema.json",
        0,
    );
}

#[test]
fn manifest_schema_matches_published_layout() {
    use athleticstandard::model::Shape;
    let id = format!(
        "https://athleticstandard.ai/schema/{}/export-manifest.schema.json",
        package::ATHLETIC_STANDARD_VERSION
    );
    compare(
        json_schema::document(package::ExportManifest::schema(), &id, "Athletic Standard export manifest"),
        "export-manifest.schema.json",
        0,
    );
}

#[test]
fn the_published_2_1_1_files_match_2_1_0_except_versions() {
    for name in ["athleticstandard.schema.json", "measurement-series.schema.json", "export-manifest.schema.json"] {
        let mut document = published("2.1.1", name);
        rewrite_versions(&mut document, "2.1.1", "2.1.0");
        first_difference(&json::to_pretty(&published("2.1.0", name)), &json::to_pretty(&document), name);
    }
}
