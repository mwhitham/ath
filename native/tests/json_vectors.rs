//! JSON text must match JavaScript's `JSON.parse` and `JSON.stringify` for every
//! recorded case. Vectors come from `native/tools/reference/generate-model-vectors.ts`.

use athleticstandard::json::{self, Value};

#[test]
fn serialization_matches_javascript() {
    let path = format!("{}/tests/vectors/json/serialization.json", env!("CARGO_MANIFEST_DIR"));
    let document = json::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = Vec::new();
    let mut refused_by_rust = Vec::new();
    for case in document["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let text = case["input_text"].as_str().unwrap();
        let parsed = json::parse(text);
        match (case["parses"].as_bool().unwrap(), parsed) {
            (true, Ok(value)) => {
                let checks = [
                    ("compact", json::to_string(&value)),
                    ("stable_compact", json::to_sorted_string(&value)),
                    ("pretty", json::to_deterministic_pretty(&value).trim_end_matches('\n').to_string()),
                ];
                for (field, actual) in checks {
                    if let Some(expected) = case.get(field).and_then(Value::as_str) {
                        if expected != actual {
                            failures.push(format!("{name} {field}: expected {expected:?}, actual {actual:?}"));
                        }
                    }
                }
            }
            (true, Err(_)) => refused_by_rust.push(name.to_string()),
            (false, Ok(_)) => failures.push(format!("{name}: JavaScript refuses this text but Rust read it")),
            (false, Err(_)) => {}
        }
    }
    // JavaScript reads these; Rust refuses them on purpose. Lone surrogates
    // cannot be held in Rust text, and a number beyond the double range would
    // become Infinity in JavaScript and `null` when written back.
    for name in &refused_by_rust {
        assert!(
            name.contains("surrogate")
                || name.contains("1e400")
                || name.contains("overflow")
                || name.contains("infinity"),
            "unexpected refusal: {name}"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
