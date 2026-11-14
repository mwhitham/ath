//! Deterministic serialization vectors (spec 2.1.1 section 9.1).
//!
//! Each vector's expected output was produced by JavaScript itself (see
//! native/tools/reference/package-golden.ts): `JSON.stringify(JSON.parse(text))`
//! and the deterministic package form. The Rust writer must produce the same
//! bytes. Where JavaScript accepts text that a Rust string cannot hold, the
//! accepted representation is an explicit refusal, never a silent change: lone
//! UTF-16 surrogates, and numbers beyond the double range, which JavaScript
//! would turn into `null`.

use std::fs;
use std::path::PathBuf;

use athleticstandard::json::{self, Value};

/// Vectors JavaScript accepts and this reader refuses on purpose.
const REFUSED: &[&str] =
    &["lone high surrogate", "lone low surrogate", "reversed surrogate pair", "number out of range"];

#[test]
fn the_package_serializer_writes_what_javascript_writes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/package/serialization-vectors.json");
    let vectors = json::parse(&fs::read_to_string(path).unwrap()).unwrap();
    let mut refused = Vec::new();
    for vector in vectors.as_array().unwrap() {
        let name = vector["name"].as_str().unwrap();
        let text = vector["text"].as_str().unwrap();
        assert_eq!(vector["accepted"], Value::Bool(true), "{name}: every vector is valid JavaScript JSON");
        match json::parse(text) {
            Ok(value) => {
                assert_eq!(json::to_string(&value), vector["compact"].as_str().unwrap(), "{name}");
                assert_eq!(json::to_deterministic_pretty(&value), vector["deterministic"].as_str().unwrap(), "{name}");
                // Writing again reads back to the same bytes.
                let again = json::parse(&json::to_string(&value)).unwrap();
                assert_eq!(json::to_deterministic_pretty(&again), vector["deterministic"].as_str().unwrap(), "{name}");
            }
            Err(_) => refused.push(name.to_string()),
        }
    }
    assert_eq!(refused, REFUSED);
}
