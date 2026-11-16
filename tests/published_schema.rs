//! The generated JSON Schema must match the published 2.1.0 files except for
//! the version list and schema IDs, because 2.1.1 changes no record field.

use athleticstandard::json::{self, Value};
use athleticstandard::model::{json_schema, package};

fn published(name: &str) -> Value {
    let path = format!("{}/schema/2.1.0/{name}", env!("CARGO_MANIFEST_DIR"));
    json::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Rewrite 2.1.1 version markers to the 2.1.0 ones for comparison.
fn as_2_1_0(value: &mut Value) {
    match value {
        Value::Array(items) => {
            let versions: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            if versions == ["1.0.0", "2.0.0", "2.1.0", "2.1.1"] {
                items.pop();
            }
            items.iter_mut().for_each(as_2_1_0);
        }
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get_mut("$id") {
                *id = id.replace("/2.1.1/", "/2.1.0/");
            }
            map.values_mut().for_each(as_2_1_0);
        }
        _ => {}
    }
}

fn compare(generated: Value, name: &str) {
    let mut generated = generated;
    as_2_1_0(&mut generated);
    let expected = json::to_pretty(&published(name));
    let actual = json::to_pretty(&generated);
    if expected != actual {
        let out = std::env::temp_dir().join(format!("generated-{name}"));
        std::fs::write(&out, &actual).unwrap();
        let first = expected.lines().zip(actual.lines()).position(|(a, b)| a != b);
        panic!("{name} differs from the published schema at line {first:?}; wrote {}", out.display());
    }
}

#[test]
fn package_schema_matches_published_layout() {
    let id = format!(
        "https://athleticstandard.ai/schema/{}/athleticstandard.schema.json",
        package::ATHLETIC_STANDARD_VERSION
    );
    compare(
        json_schema::document(package::package_schema(), &id, "Athletic Standard portable package document"),
        "athleticstandard.schema.json",
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
    );
}
