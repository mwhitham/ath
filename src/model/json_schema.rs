//! Generate the published JSON Schema (draft 2020-12) from the model shapes.
//!
//! The output layout matches the files published for 2.1.0, so a reader can
//! compare versions line by line. Refinements that JSON Schema cannot express
//! are listed in `build-history/v2.1.1/appendix/semantic-rules.md` and run in the Rust
//! validator; structural validity does not prove a package's history is valid.

use super::schema::{Check, Kind, Schema, SchemaRef, StringFormat};
use super::time::{CALENDAR_DATE_PATTERN, UTC_TIMESTAMP_PATTERN};
use crate::json::{Map, Value};

/// A JSON Schema document with its identity.
pub fn document(schema: SchemaRef, id: &str, title: &str) -> Value {
    let mut definitions = Definitions::default();
    let body = emit(schema, &mut definitions);
    let mut out = Map::new();
    out.insert("$schema".into(), "https://json-schema.org/draft/2020-12/schema".into());
    out.insert("$id".into(), id.into());
    out.insert("title".into(), title.into());
    if let Value::Object(map) = body {
        for (key, value) in map {
            out.insert(key, value);
        }
    }
    if !definitions.entries.is_empty() {
        let mut defs = Map::new();
        for (name, value) in definitions.entries {
            defs.insert(name, value);
        }
        out.insert("$defs".into(), Value::Object(defs));
    }
    Value::Object(out)
}

#[derive(Default)]
struct Definitions {
    /// Recursive shapes in first-use order: (kind tag, name).
    names: Vec<(&'static str, String)>,
    entries: Vec<(String, Value)>,
    /// For `Components`: where references point, and the shapes that have a name.
    /// A standalone document leaves these empty and uses `#/$defs/`.
    prefix: Option<String>,
    named: Vec<(usize, String)>,
}

impl Definitions {
    fn target(&self, name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}{name}"),
            None => format!("#/$defs/{name}"),
        }
    }

    fn reference(&mut self, tag: &'static str, build: fn(&str) -> Value) -> Value {
        let name = match self.names.iter().find(|(known, _)| *known == tag) {
            Some((_, name)) => name.clone(),
            None => {
                let name = match (&self.prefix, tag) {
                    (None, _) => format!("__schema{}", self.names.len()),
                    (Some(_), "json") => "JsonValue".to_string(),
                    (Some(_), "extension") => "ExtensionValue".to_string(),
                    (Some(_), other) => other.to_string(),
                };
                self.names.push((tag, name.clone()));
                let body = build(&self.target(&name));
                self.entries.push((name.clone(), body));
                name
            }
        };
        serde_json::json!({ "$ref": self.target(&name) })
    }

    fn named(&self, schema: &Schema) -> Option<Value> {
        let address = schema as *const Schema as usize;
        self.named
            .iter()
            .find(|(known, _)| *known == address)
            .map(|(_, name)| serde_json::json!({ "$ref": self.target(name) }))
    }
}

/// Shapes written as parts of one larger document, such as an API description.
/// A shape given a name is written once, under that name, and every other use of
/// it becomes a reference to `prefix` and the name. Shared recursive shapes are
/// named `JsonValue` and `ExtensionValue`.
pub struct Components {
    definitions: Definitions,
}

impl Components {
    pub fn new(prefix: &str) -> Components {
        Components { definitions: Definitions { prefix: Some(prefix.to_string()), ..Definitions::default() } }
    }

    /// Give `schema` a name. Every later use of it inside another shape becomes a
    /// reference.
    pub fn name(&mut self, schema: SchemaRef, name: &str) {
        self.definitions.named.push((schema as *const Schema as usize, name.to_string()));
    }

    /// `schema` written out in full, with named shapes inside it as references.
    pub fn write(&mut self, schema: SchemaRef) -> Value {
        emit_body(schema, &mut self.definitions)
    }

    /// The reference to a named shape, or the shape in full if it has no name.
    pub fn use_of(&mut self, schema: SchemaRef) -> Value {
        emit(schema, &mut self.definitions)
    }

    /// The shared recursive shapes the written shapes used, by name.
    pub fn shared(self) -> Vec<(String, Value)> {
        self.definitions.entries
    }
}

fn json_union(target: &str, string: Value, array_max: Option<usize>) -> Value {
    let reference = serde_json::json!({ "$ref": target });
    let mut array = Map::new();
    if let Some(limit) = array_max {
        array.insert("maxItems".into(), Value::from(limit as u64));
    }
    array.insert("type".into(), "array".into());
    array.insert("items".into(), reference.clone());
    serde_json::json!({
        "anyOf": [
            string,
            { "type": "number" },
            { "type": "boolean" },
            { "type": "null" },
            Value::Object(array),
            { "type": "object", "propertyNames": { "type": "string" }, "additionalProperties": reference }
        ]
    })
}

fn extension_value(target: &str) -> Value {
    json_union(
        target,
        serde_json::json!({ "type": "string", "maxLength": super::extensions::EXTENSION_STRING_MAX_LENGTH }),
        Some(super::extensions::EXTENSION_ARRAY_MAX_LENGTH),
    )
}

fn any_json(target: &str) -> Value {
    json_union(target, serde_json::json!({ "type": "string" }), None)
}

fn number(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() <= super::schema::MAX_SAFE_INTEGER {
        Value::from(value as i64)
    } else {
        serde_json::Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
    }
}

fn emit(schema: &Schema, definitions: &mut Definitions) -> Value {
    if let Some(reference) = definitions.named(schema) {
        return reference;
    }
    emit_body(schema, definitions)
}

fn emit_body(schema: &Schema, definitions: &mut Definitions) -> Value {
    let mut out = Map::new();
    if schema.description_first {
        if let Some(description) = schema.description {
            out.insert("description".into(), description.into());
        }
    }
    match &schema.kind {
        Kind::String => {
            out.insert("type".into(), "string".into());
            for check in &schema.checks {
                match check {
                    Check::MinLength(length) => {
                        out.insert("minLength".into(), Value::from(*length as u64));
                    }
                    Check::MaxLength(length) => {
                        out.insert("maxLength".into(), Value::from(*length as u64));
                    }
                    Check::Format(StringFormat::DateTimeMillisUtc) => {
                        out.insert("format".into(), "date-time".into());
                        out.insert("pattern".into(), UTC_TIMESTAMP_PATTERN.into());
                    }
                    Check::Format(StringFormat::Date) => {
                        out.insert("format".into(), "date".into());
                        out.insert("pattern".into(), CALENDAR_DATE_PATTERN.into());
                    }
                    Check::Pattern { source, .. } => {
                        out.insert("pattern".into(), (*source).into());
                    }
                    _ => {}
                }
            }
        }
        Kind::Number { integer } => {
            out.insert("type".into(), if *integer { "integer" } else { "number" }.into());
            let mut minimum: Option<(f64, bool)> = None;
            let mut maximum: Option<(f64, bool)> = None;
            for check in &schema.checks {
                match check {
                    Check::Minimum { value, inclusive } => minimum = Some((*value, *inclusive)),
                    Check::Maximum { value, inclusive } => maximum = Some((*value, *inclusive)),
                    _ => {}
                }
            }
            if *integer {
                minimum = minimum.or(Some((-super::schema::MAX_SAFE_INTEGER, true)));
                maximum = maximum.or(Some((super::schema::MAX_SAFE_INTEGER, true)));
            }
            if let Some((value, inclusive)) = minimum {
                out.insert(if inclusive { "minimum" } else { "exclusiveMinimum" }.into(), number(value));
            }
            if let Some((value, inclusive)) = maximum {
                out.insert(if inclusive { "maximum" } else { "exclusiveMaximum" }.into(), number(value));
            }
        }
        Kind::Boolean => {
            out.insert("type".into(), "boolean".into());
        }
        Kind::Null => {
            out.insert("type".into(), "null".into());
        }
        Kind::Enum(options) => {
            out.insert("type".into(), "string".into());
            out.insert("enum".into(), Value::Array(options.iter().map(|option| Value::from(*option)).collect()));
        }
        Kind::Literal(value) => {
            out.insert("type".into(), "string".into());
            out.insert("const".into(), (*value).into());
        }
        Kind::Object { fields } => {
            out.insert("type".into(), "object".into());
            let mut properties = Map::new();
            let mut required = Vec::new();
            for field in fields {
                properties.insert(field.name.into(), emit(field.schema, definitions));
                if !field.optional {
                    required.push(Value::from(field.name));
                }
            }
            out.insert("properties".into(), Value::Object(properties));
            if !required.is_empty() {
                out.insert("required".into(), Value::Array(required));
            }
            out.insert("additionalProperties".into(), Value::Bool(false));
        }
        Kind::Array(items) => {
            for check in &schema.checks {
                if let Check::MaxItems(limit) = check {
                    out.insert("maxItems".into(), Value::from(*limit as u64));
                }
            }
            out.insert("type".into(), "array".into());
            out.insert("items".into(), emit(items, definitions));
        }
        Kind::Record { key, value } => {
            out.insert("type".into(), "object".into());
            out.insert("propertyNames".into(), emit(key, definitions));
            out.insert("additionalProperties".into(), emit(value, definitions));
        }
        Kind::Union(options) => {
            out.insert("anyOf".into(), Value::Array(options.iter().map(|option| emit(option, definitions)).collect()));
        }
        Kind::Discriminated { options, .. } => {
            out.insert(
                "oneOf".into(),
                Value::Array(options.iter().map(|(_, option)| emit(option, definitions)).collect()),
            );
        }
        Kind::Nullable(inner) => {
            out.insert(
                "anyOf".into(),
                Value::Array(vec![emit(inner, definitions), serde_json::json!({ "type": "null" })]),
            );
        }
        Kind::Json => return definitions.reference("json", any_json),
        Kind::ExtensionValue => return definitions.reference("extension", extension_value),
    }
    if let (Some(description), false) = (schema.description, schema.description_first) {
        out.insert("description".into(), description.into());
    }
    Value::Object(out)
}
