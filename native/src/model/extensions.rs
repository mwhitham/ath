//! Optional namespaced extensions on records (D118).
//!
//! An extension value inherits its record's class. Extensions on
//! athlete-reported records cannot contain `source_id` at any depth, so they
//! cannot claim a measured source. Size limits keep one record's extensions
//! small enough to store and review.

use std::sync::LazyLock;

use super::schema::{custom, leak, Check, Issue, IssueCode, Kind, PathSegment, Schema, SchemaRef};
use crate::json::{self, Value};

/// Maximum UTF-8 bytes for the whole extensions object when JSON-encoded.
pub const EXTENSIONS_MAX_ENCODED_BYTES: usize = 65_536;
/// Maximum UTF-8 bytes for one extension value when JSON-encoded.
pub const EXTENSION_VALUE_MAX_ENCODED_BYTES: usize = 16_384;
/// Maximum extension keys on one record.
pub const EXTENSIONS_MAX_KEYS: usize = 32;
/// Maximum nesting depth for one extension value.
pub const EXTENSION_VALUE_MAX_DEPTH: usize = 8;
/// Maximum UTF-16 length of one extension string.
pub const EXTENSION_STRING_MAX_LENGTH: usize = 4_096;
/// Maximum elements in one extension array.
pub const EXTENSION_ARRAY_MAX_LENGTH: usize = 256;
/// Maximum keys in one extension object.
pub const EXTENSION_OBJECT_MAX_KEYS: usize = 64;

/// Record types whose extensions cannot name a source.
pub const ATHLETE_REPORTED_RECORD_TYPES: &[&str] =
    &["athlete_report", "nutrition_entry", "attachment", "athlete_profile"];

/// The published extension ID pattern: owner segments, slash, name.
pub const EXTENSION_ID_PATTERN: &str = r"^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)+\/[a-z][a-z0-9][a-z0-9._-]*$";

/// A valid extension key: the published pattern and at most 200 characters.
pub fn is_extension_id(text: &str) -> bool {
    text.chars().count() <= 200 && is_extension_id_pattern(text)
}

/// The published extension ID pattern: dotted lowercase owner, slash, name.
pub fn is_extension_id_pattern(text: &str) -> bool {
    let Some(slash) = text.find('/') else {
        return false;
    };
    let (owner, name) = (&text[..slash], &text[slash + 1..]);
    let segments: Vec<&str> = owner.split('.').collect();
    if segments.len() < 2 || !segments.iter().all(|segment| is_owner_segment(segment)) {
        return false;
    }
    let bytes = name.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_lowercase()
        && (bytes[1].is_ascii_lowercase() || bytes[1].is_ascii_digit())
        && bytes[2..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_owner_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_lowercase()
        && bytes[1..].iter().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// The options of the extension value union, in published order: a string of
/// at most 4,096 characters, a number, a boolean, null, an array of at most 256
/// extension values, or an object of extension values.
pub fn extension_value_options() -> &'static [SchemaRef] {
    static OPTIONS: LazyLock<Vec<SchemaRef>> = LazyLock::new(|| {
        let value = leak(Schema::new(Kind::ExtensionValue));
        let key = leak(Schema::new(Kind::String));
        vec![
            leak(Schema::new(Kind::String).check(Check::MaxLength(EXTENSION_STRING_MAX_LENGTH))),
            leak(Schema::new(Kind::Number { integer: false })),
            leak(Schema::new(Kind::Boolean)),
            leak(Schema::new(Kind::Null)),
            leak(Schema::new(Kind::Array(value)).check(Check::MaxItems(EXTENSION_ARRAY_MAX_LENGTH))),
            leak(Schema::new(Kind::Record { key, value })),
        ]
    });
    &OPTIONS
}

/// The shape of a record's `extensions` object.
pub fn record_extensions() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> = LazyLock::new(|| {
        let value = leak(Schema::new(Kind::ExtensionValue));
        let key = leak(Schema::new(Kind::String));
        leak(Schema::new(Kind::Record { key, value }).check(Check::Refine(refine_extensions)))
    });
    &SCHEMA
}

fn refine_extensions(value: &Value, issues: &mut Vec<Issue>) {
    let Value::Object(map) = value else {
        return;
    };
    for key in map.keys() {
        if !is_extension_id(key) {
            issues.push(Issue {
                path: vec![PathSegment::Key(key.clone())],
                code: IssueCode::Custom,
                message: "extension key must be a namespaced extension id".into(),
            });
        }
    }
    if map.len() > EXTENSIONS_MAX_KEYS {
        custom(issues, &[], format!("extensions cannot contain more than {EXTENSIONS_MAX_KEYS} keys"));
        return;
    }
    if json::to_string(value).len() > EXTENSIONS_MAX_ENCODED_BYTES {
        custom(issues, &[], format!("extensions exceed {EXTENSIONS_MAX_ENCODED_BYTES} encoded bytes"));
    }
    for key in json::ordered_keys(map, false) {
        let item = &map[key.as_str()];
        if json::to_string(item).len() > EXTENSION_VALUE_MAX_ENCODED_BYTES {
            issues.push(Issue {
                path: vec![PathSegment::Key(key.clone())],
                code: IssueCode::Custom,
                message: format!("extension value exceeds {EXTENSION_VALUE_MAX_ENCODED_BYTES} encoded bytes"),
            });
        }
        let mut shape_issues = Vec::new();
        shape_check(item, key.clone(), 1, &mut shape_issues);
        for (path, message) in shape_issues {
            // The published validator builds a dotted path string and splits
            // it on `.`, `[`, and `]`. An extension ID that contains dots is
            // therefore split too. This keeps the same issue paths.
            let segments = path
                .split(['.', '[', ']'])
                .filter(|segment| !segment.is_empty())
                .map(|segment| PathSegment::Key(segment.to_string()))
                .collect();
            issues.push(Issue { path: segments, code: IssueCode::Custom, message });
        }
    }
}

fn shape_check(value: &Value, path: String, depth: usize, issues: &mut Vec<(String, String)>) {
    if depth > EXTENSION_VALUE_MAX_DEPTH {
        issues.push((path, format!("extension value exceeds depth {EXTENSION_VALUE_MAX_DEPTH}")));
        return;
    }
    match value {
        Value::String(text) => {
            if json::utf16_len(text) > EXTENSION_STRING_MAX_LENGTH {
                issues.push((path, format!("extension string exceeds {EXTENSION_STRING_MAX_LENGTH} characters")));
            }
        }
        Value::Array(items) => {
            if items.len() > EXTENSION_ARRAY_MAX_LENGTH {
                issues.push((path.clone(), format!("extension array exceeds {EXTENSION_ARRAY_MAX_LENGTH} elements")));
            }
            for (index, item) in items.iter().enumerate() {
                shape_check(item, format!("{path}[{index}]"), depth + 1, issues);
            }
        }
        Value::Object(map) => {
            if map.len() > EXTENSION_OBJECT_MAX_KEYS {
                issues.push((path.clone(), format!("extension object exceeds {EXTENSION_OBJECT_MAX_KEYS} keys")));
            }
            for key in json::ordered_keys(map, false) {
                let child = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                shape_check(&map[key.as_str()], child, depth + 1, issues);
            }
        }
        _ => {}
    }
}

/// Athlete-reported records cannot carry `source_id` anywhere in extensions.
/// Returns the first problem, as the published check does.
pub fn source_claim_problem(record_type: &str, extensions: Option<&Value>) -> Option<&'static str> {
    let extensions = extensions?;
    if !ATHLETE_REPORTED_RECORD_TYPES.contains(&record_type) {
        return None;
    }
    if contains_key(extensions, "source_id") {
        Some("extensions cannot contain source_id")
    } else {
        None
    }
}

fn contains_key(value: &Value, name: &str) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, child)| key == name || contains_key(child, name)),
        Value::Array(items) => items.iter().any(|item| contains_key(item, name)),
        _ => false,
    }
}
