//! The npm package's generated files must match their native sources.
//!
//! - `packages/npm/src/generated/protocol.ts` comes from the protocol's
//!   operation definitions.
//! - `packages/npm/schema/2.1.1/*.json` come from the Rust model.
//!
//! A failing test names the file that differs. `ATH_WRITE_GENERATED=1` writes
//! the generated text instead of comparing it.

use std::path::PathBuf;

use athleticstandard::json;
use athleticstandard::model::{json_schema, package, Shape};
use athleticstandard::protocol::typescript;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn check(relative: &str, expected: &str) {
    let path = repository_root().join(relative);
    if std::env::var("ATH_WRITE_GENERATED").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, expected).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    if committed != expected {
        let first = committed.lines().zip(expected.lines()).position(|(a, b)| a != b);
        panic!(
            "{relative} does not match its generator (first differing line {first:?}). \
             Run `ATH_WRITE_GENERATED=1 cargo test --test npm_generated` and commit the result."
        );
    }
}

#[test]
fn protocol_module_matches_the_operation_definitions() {
    check(typescript::GENERATED_PATH, &typescript::generate());
}

fn schema_text(schema: athleticstandard::model::schema::SchemaRef, file: &str, title: &str) -> String {
    let id = format!("https://athleticstandard.ai/schema/{}/{file}", package::ATHLETIC_STANDARD_VERSION);
    let mut text = json::to_pretty(&json_schema::document(schema, &id, title));
    text.push('\n');
    text
}

#[test]
fn json_schemas_match_the_model() {
    let version = package::ATHLETIC_STANDARD_VERSION;
    let documents = [
        (
            "athleticstandard.schema.json",
            schema_text(
                package::package_schema(),
                "athleticstandard.schema.json",
                "Athletic Standard portable package document",
            ),
        ),
        (
            "export-manifest.schema.json",
            schema_text(
                package::ExportManifest::schema(),
                "export-manifest.schema.json",
                "Athletic Standard export manifest",
            ),
        ),
        (
            "measurement-series.schema.json",
            schema_text(
                package::measurement_series_file_schema(),
                "measurement-series.schema.json",
                "Athletic Standard measurement-series file",
            ),
        ),
    ];
    for (file, text) in documents {
        check(&format!("packages/npm/schema/{version}/{file}"), &text);
    }
}
