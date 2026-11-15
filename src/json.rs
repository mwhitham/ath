//! JSON text that matches JavaScript's `JSON.parse` and `JSON.stringify`.
//!
//! Stored record JSON and package files were written by JavaScript. The bytes
//! must stay the same when this library writes the same logical value, so the
//! writer follows the ECMAScript rules instead of Rust defaults:
//!
//! - Every number is an IEEE double and prints like `Number.prototype.toString`.
//!   `100.0` prints `100`, `1e21` prints `1e+21`, and `-0` prints `0`.
//! - An object lists array-index keys (`"0"`, `"10"`) first in numeric order,
//!   then the other keys in insertion order. JavaScript objects always do this.
//! - Strings escape only `"`, `\`, and control characters below U+0020.
//!
//! Deterministic package files also sort the remaining keys by UTF-16 code
//! units, which is JavaScript's string order.

use std::cmp::Ordering;
use std::fmt::Write as _;

pub use serde_json::{Map, Number, Value};

/// A JSON text could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for JsonError {}

/// Read JSON text. Duplicate keys keep the first position and the last value,
/// as `JSON.parse` does.
///
/// JavaScript accepts a lone UTF-16 surrogate escape such as `"\ud800"`. Rust
/// strings cannot hold one, so this reader refuses that text with an error.
pub fn parse(text: &str) -> Result<Value, JsonError> {
    serde_json::from_str::<Value>(text).map_err(|error| JsonError { message: format!("Invalid JSON: {error}") })
}

/// Read JSON bytes. The bytes must be UTF-8.
pub fn parse_bytes(bytes: &[u8]) -> Result<Value, JsonError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| JsonError { message: "Invalid JSON: text is not UTF-8".to_string() })?;
    parse(text)
}

/// `JSON.stringify(value)`.
pub fn to_string(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, KeyOrder::Insertion, None, 0);
    out
}

/// `JSON.stringify(stableValue(value), null, 2) + "\n"`: the deterministic
/// package encoding. Object keys are sorted by UTF-16 code units after the
/// array-index keys.
pub fn to_deterministic_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, KeyOrder::Sorted, Some(2), 0);
    out.push('\n');
    out
}

/// `JSON.stringify(stableValue(value))`: sorted keys without indentation.
pub fn to_sorted_string(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, KeyOrder::Sorted, None, 0);
    out
}

/// `JSON.stringify(value, null, 2)` without sorting.
pub fn to_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, KeyOrder::Insertion, Some(2), 0);
    out
}

/// Serialize a typed value with JavaScript formatting.
pub fn encode<T: serde::Serialize + ?Sized>(value: &T) -> String {
    to_string(&to_value(value))
}

/// Convert a typed value into a JSON value. Typed values in this library
/// always serialize; a failure is a programming error.
pub fn to_value<T: serde::Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).expect("library values always serialize to JSON")
}

/// True when two JSON values are the same after JavaScript normalization and
/// key sorting. Used to compare stored JSON columns.
pub fn semantic_eq(left: &Value, right: &Value) -> bool {
    to_sorted_string(left) == to_sorted_string(right)
}

/// Format a double as `Number.prototype.toString` does. JSON has no NaN or
/// infinity; `JSON.stringify` writes `null` for them.
pub fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "null".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    let mut buffer = ryu_js::Buffer::new();
    buffer.format_finite(value).to_string()
}

/// The double JavaScript holds for a JSON number.
pub fn number_as_f64(number: &Number) -> f64 {
    if let Some(value) = number.as_f64() {
        value
    } else if let Some(value) = number.as_i64() {
        value as f64
    } else {
        number.as_u64().map(|value| value as f64).unwrap_or(f64::NAN)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KeyOrder {
    Insertion,
    Sorted,
}

/// The ECMAScript array-index test: canonical decimal text below 2^32 - 1.
pub fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 {
        return None;
    }
    let bytes = key.as_bytes();
    if !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if bytes.len() > 1 && bytes[0] == b'0' {
        return None;
    }
    let value: u64 = key.parse().ok()?;
    if value >= u64::from(u32::MAX) {
        return None;
    }
    Some(value as u32)
}

/// Compare strings by UTF-16 code units, which is JavaScript's `<` on strings.
pub fn compare_utf16(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

/// Keys in the order a JavaScript object would list them.
pub fn ordered_keys(map: &Map<String, Value>, sorted: bool) -> Vec<&String> {
    let mut indexes: Vec<(u32, &String)> = Vec::new();
    let mut names: Vec<&String> = Vec::new();
    for key in map.keys() {
        match array_index(key) {
            Some(index) => indexes.push((index, key)),
            None => names.push(key),
        }
    }
    indexes.sort_by_key(|(index, _)| *index);
    if sorted {
        names.sort_by(|left, right| compare_utf16(left, right));
    }
    indexes.into_iter().map(|(_, key)| key).chain(names).collect()
}

fn write_value(out: &mut String, value: &Value, order: KeyOrder, indent: Option<usize>, depth: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&format_number(number_as_f64(number))),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_value(out, item, order, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (index, key) in ordered_keys(map, order == KeyOrder::Sorted).into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_string(out, key);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_value(out, &map[key.as_str()], order, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, indent: Option<usize>, depth: usize) {
    if let Some(width) = indent {
        out.push('\n');
        for _ in 0..width * depth {
            out.push(' ');
        }
    }
}

/// `JSON.stringify` string quoting.
pub fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Length of a string in UTF-16 code units, which is JavaScript's `length`.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(text: &str) -> String {
        to_string(&parse(text).unwrap())
    }

    #[test]
    fn numbers_print_like_javascript() {
        assert_eq!(round("100.0"), "100");
        assert_eq!(round("1E2"), "100");
        assert_eq!(round("-0"), "0");
        assert_eq!(round("-0.0"), "0");
        assert_eq!(round("0.1"), "0.1");
        assert_eq!(round("1e21"), "1e+21");
        assert_eq!(round("1e-7"), "1e-7");
        assert_eq!(round("0.000001"), "0.000001");
        assert_eq!(round("123456789012345678901"), "123456789012345680000");
        assert_eq!(round("12345678901234567890"), "12345678901234567000");
        assert_eq!(round("5e-324"), "5e-324");
        assert_eq!(round("1.7976931348623157e308"), "1.7976931348623157e+308");
        assert_eq!(round("[0.30000000000000004]"), "[0.30000000000000004]");
    }

    #[test]
    fn array_index_keys_come_first() {
        assert_eq!(
            round(r#"{"z":1,"10":2,"a":3,"2":4,"01":5,"4294967295":6,"4294967294":7}"#),
            r#"{"2":4,"10":2,"4294967294":7,"z":1,"a":3,"01":5,"4294967295":6}"#
        );
    }

    #[test]
    fn duplicate_keys_keep_first_position_and_last_value() {
        assert_eq!(round(r#"{"a":1,"b":2,"a":3}"#), r#"{"a":3,"b":2}"#);
    }

    #[test]
    fn strings_escape_like_javascript() {
        assert_eq!(round(r#""\u0001\u001f\u007f \/\b\f\n\r\t""#), "\"\\u0001\\u001f\u{7f}\u{2028}/\\b\\f\\n\\r\\t\"");
    }

    #[test]
    fn deterministic_pretty_sorts_by_utf16() {
        let value = parse(r#"{"😀":1,"ﬁ":2,"b":{"y":[],"x":{}},"a":[1,{"d":1,"c":2}]}"#).unwrap();
        assert_eq!(
            to_deterministic_pretty(&value),
            "{\n  \"a\": [\n    1,\n    {\n      \"c\": 2,\n      \"d\": 1\n    }\n  ],\n  \"b\": {\n    \"x\": {},\n    \"y\": []\n  },\n  \"😀\": 1,\n  \"ﬁ\": 2\n}\n"
        );
    }

    #[test]
    fn lone_surrogates_are_refused() {
        assert!(parse(r#""\ud800""#).is_err());
    }
}
