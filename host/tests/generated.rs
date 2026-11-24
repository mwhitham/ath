//! The host API description, generated from the operation table and the body shapes
//! (D266). `cargo test --test generated` runs this check with the library's
//! generated-file checks, and `ATH_WRITE_GENERATED=1 cargo test --test generated`
//! rewrites the committed copy.

use std::fs;
use std::path::PathBuf;

use ath_host::description::{generate, GENERATED_PATH};

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("the repository folder").to_path_buf()
}

#[test]
fn the_api_description_matches_the_operations() {
    let expected = generate();
    let path = repository().join(GENERATED_PATH);
    if std::env::var("ATH_WRITE_GENERATED").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &expected).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    if committed != expected {
        let first = committed.lines().zip(expected.lines()).position(|(a, b)| a != b);
        panic!(
            "{GENERATED_PATH} does not match its generator (first differing line {first:?}). \
             Run `ATH_WRITE_GENERATED=1 cargo test --test generated` and commit the result."
        );
    }
}

#[test]
fn the_api_description_points_at_published_record_schemas() {
    let description: serde_json::Value = serde_json::from_str(&generate()).unwrap();
    let schemas = &description["components"]["schemas"];
    for name in ["CurrentRecord", "RecordRevision"] {
        for option in schemas[name]["anyOf"].as_array().unwrap() {
            let reference = option["$ref"].as_str().unwrap();
            let (file, pointer) = reference.split_once('#').unwrap();
            let target = repository().join(GENERATED_PATH).parent().unwrap().join(file);
            let schema: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&target).unwrap_or_else(|_| panic!("{reference}"))).unwrap();
            assert!(schema.pointer(pointer).is_some(), "{reference} points at nothing");
        }
    }
}
