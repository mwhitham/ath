//! Operation handlers, grouped by the library area they call.
//!
//! Handlers adapt request input to library calls and library results to the
//! 2.1.0 JavaScript result shapes. They hold no SQL and no domain rule of
//! their own; checks that 2.1.0 made on arguments use the published shapes.

mod database;
mod helpers;
mod package;
mod prediction;
mod query;
mod records;

use super::definition::Operation;

/// Every operation, in a stable order.
pub fn all() -> Vec<Operation> {
    let mut out = Vec::new();
    out.extend_from_slice(helpers::OPERATIONS);
    out.extend_from_slice(database::OPERATIONS);
    out.extend_from_slice(records::OPERATIONS);
    out.extend_from_slice(query::OPERATIONS);
    out.extend_from_slice(prediction::OPERATIONS);
    out.extend_from_slice(package::OPERATIONS);
    let mut names: Vec<&str> = out.iter().map(|operation| operation.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), out.len(), "operation names must be unique");
    out
}

pub(crate) mod input {
    //! Reading request input.

    use crate::error::Error;
    use crate::json::Value;
    use crate::model::schema::{self, SchemaRef};
    use crate::model::{registry, Shape};
    use crate::protocol::worker::Failure;

    /// A present, non-null field.
    pub fn get<'a>(input: &'a Value, key: &str) -> Option<&'a Value> {
        input.get(key).filter(|value| !value.is_null())
    }

    /// A required field, as the client always sends it.
    pub fn required<'a>(input: &'a Value, key: &str) -> Result<&'a Value, Failure> {
        get(input, key).ok_or_else(|| Failure::Input(format!("input.{key} is missing")))
    }

    pub fn string(input: &Value, key: &str) -> Result<Option<String>, Failure> {
        match get(input, key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(Failure::Input(format!("input.{key} must be a string"))),
        }
    }

    pub fn required_string(input: &Value, key: &str) -> Result<String, Failure> {
        string(input, key)?.ok_or_else(|| Failure::Input(format!("input.{key} is missing")))
    }

    pub fn flag(input: &Value, key: &str) -> Result<bool, Failure> {
        match get(input, key) {
            None => Ok(false),
            Some(Value::Bool(value)) => Ok(*value),
            Some(_) => Err(Failure::Input(format!("input.{key} must be a boolean"))),
        }
    }

    pub fn array<'a>(input: &'a Value, key: &str) -> Result<&'a [Value], Failure> {
        match get(input, key) {
            None => Ok(&[]),
            Some(Value::Array(items)) => Ok(items),
            Some(_) => Err(Failure::Input(format!("input.{key} must be an array"))),
        }
    }

    /// A JavaScript number that is a non-negative integer, as JSON carries it.
    pub fn whole_number(value: &Value) -> Option<u64> {
        let number = value.as_f64()?;
        (number.is_finite() && number >= 0.0 && number.fract() == 0.0 && number <= 9_007_199_254_740_991.0)
            .then_some(number as u64)
    }

    /// The published shape with this export name.
    pub fn shape(name: &str) -> SchemaRef {
        registry::shape(name).unwrap_or_else(|| panic!("no published shape named {name}"))
    }

    /// Check a value with a published shape, as `Schema.parse` did.
    pub fn check(name: &str, value: &Value) -> Result<Value, Failure> {
        let schema = shape(name);
        schema.validate(value)?;
        Ok(schema.output(value))
    }

    /// Check a value with a published shape and decode it.
    pub fn decode<T: Shape + serde::de::DeserializeOwned>(value: &Value) -> Result<T, Failure> {
        Ok(schema::decode::<T>(value)?)
    }

    /// The text of a checked string value.
    pub fn checked_text(name: &str, value: &Value) -> Result<String, Failure> {
        match check(name, value)? {
            Value::String(text) => Ok(text),
            _ => Err(Failure::Error(Error::other(format!("{name} is not text")))),
        }
    }

    /// `value ?? fallback` for a string shape: a missing value takes the
    /// fallback before it is checked.
    pub fn checked_text_or(
        name: &str,
        value: Option<&Value>,
        fallback: impl FnOnce() -> String,
    ) -> Result<String, Failure> {
        match value {
            Some(value) => checked_text(name, value),
            None => checked_text(name, &Value::String(fallback())),
        }
    }
}
