//! The structural description of every record shape.
//!
//! Each model type declares its shape once (see `model_object!`). The same
//! description validates input, generates the published JSON Schema, and
//! generates TypeScript declarations. Validation follows the published 2.1.x
//! rules exactly, including the order, paths, and wording of issues, so a
//! person or agent sees the same error for the same mistake.
//!
//! Issue rules that match the published validator:
//!
//! - A wrong type, an invalid enum value, a failed union, or an invalid record
//!   key stops further checks on the parent object ("aborting" issues).
//! - Length, range, format, unknown-key, and custom issues let later checks run.
//! - Object refinements run only when no aborting issue occurred inside them.

use std::sync::LazyLock;

use crate::json::{self, Value};

/// The largest integer a JavaScript number holds exactly.
pub const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// One step in the location of an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

impl std::fmt::Display for PathSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathSegment::Key(key) => f.write_str(key),
            PathSegment::Index(index) => write!(f, "{index}"),
        }
    }
}

impl From<&str> for PathSegment {
    fn from(value: &str) -> Self {
        PathSegment::Key(value.to_string())
    }
}

impl From<usize> for PathSegment {
    fn from(value: usize) -> Self {
        PathSegment::Index(value)
    }
}

/// The kind of validation issue. The names match the published issue codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCode {
    InvalidType,
    InvalidValue,
    InvalidUnion,
    InvalidKey,
    InvalidFormat,
    TooSmall,
    TooBig,
    UnrecognizedKeys,
    Custom,
}

impl IssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueCode::InvalidType => "invalid_type",
            IssueCode::InvalidValue => "invalid_value",
            IssueCode::InvalidUnion => "invalid_union",
            IssueCode::InvalidKey => "invalid_key",
            IssueCode::InvalidFormat => "invalid_format",
            IssueCode::TooSmall => "too_small",
            IssueCode::TooBig => "too_big",
            IssueCode::UnrecognizedKeys => "unrecognized_keys",
            IssueCode::Custom => "custom",
        }
    }

    /// Aborting issues stop parent refinements.
    pub fn aborts(self) -> bool {
        matches!(
            self,
            IssueCode::InvalidType | IssueCode::InvalidValue | IssueCode::InvalidUnion | IssueCode::InvalidKey
        )
    }
}

/// One validation problem with its location.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub path: Vec<PathSegment>,
    pub code: IssueCode,
    pub message: String,
}

impl Issue {
    /// `data.value.standard_unit`, or `record` for the top level.
    pub fn path_text(&self) -> String {
        if self.path.is_empty() {
            "record".to_string()
        } else {
            self.path.iter().map(ToString::to_string).collect::<Vec<_>>().join(".")
        }
    }

    pub fn to_json(&self) -> Value {
        let path: Vec<Value> = self
            .path
            .iter()
            .map(|segment| match segment {
                PathSegment::Key(key) => Value::String(key.clone()),
                PathSegment::Index(index) => Value::from(*index as u64),
            })
            .collect();
        serde_json::json!({ "path": path, "code": self.code.as_str(), "message": self.message })
    }
}

/// A value did not match its shape. The first issue is the one shown to a person.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub issues: Vec<Issue>,
}

impl ValidationError {
    pub fn single(path: Vec<PathSegment>, message: impl Into<String>) -> Self {
        ValidationError { issues: vec![Issue { path, code: IssueCode::Custom, message: message.into() }] }
    }

    /// Put every issue under a parent path, for example `data`.
    pub fn under(mut self, prefix: &[PathSegment]) -> Self {
        for issue in &mut self.issues {
            let mut path = prefix.to_vec();
            path.append(&mut issue.path);
            issue.path = path;
        }
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.issues.first() {
            Some(issue) => write!(f, "{}: {}", issue.path_text(), issue.message),
            None => f.write_str("record does not match its domain schema"),
        }
    }
}

impl std::error::Error for ValidationError {}

pub type SchemaRef = &'static Schema;

/// A string format with a fixed published pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringFormat {
    /// UTC timestamp with exactly three fractional digits and `Z`.
    DateTimeMillisUtc,
    /// Calendar date `YYYY-MM-DD`.
    Date,
}

/// One check on a value that already has the right type.
pub enum Check {
    MinLength(usize),
    MaxLength(usize),
    Format(StringFormat),
    /// A regular-expression check. `source` is the published pattern; `test` is
    /// the equivalent hand-written test.
    Pattern {
        source: &'static str,
        test: fn(&str) -> bool,
        message: &'static str,
    },
    /// A string refinement that does not appear in JSON Schema.
    StringRefine {
        test: fn(&str) -> bool,
        message: &'static str,
    },
    Minimum {
        value: f64,
        inclusive: bool,
    },
    Maximum {
        value: f64,
        inclusive: bool,
    },
    MaxItems(usize),
    /// A refinement over the whole value. It appends issues with paths
    /// relative to the value.
    Refine(fn(&Value, &mut Vec<Issue>)),
}

pub enum Kind {
    String,
    Number {
        integer: bool,
    },
    Boolean,
    Null,
    Enum(&'static [&'static str]),
    Literal(&'static str),
    Object {
        fields: Vec<Field>,
    },
    Array(SchemaRef),
    Record {
        key: SchemaRef,
        value: SchemaRef,
    },
    Union(Vec<SchemaRef>),
    Discriminated {
        key: &'static str,
        options: Vec<(&'static str, SchemaRef)>,
    },
    Nullable(SchemaRef),
    /// Any JSON value (`z.json()`).
    Json,
    /// A bounded extension value. See `model::extensions`.
    ExtensionValue,
}

pub struct Field {
    pub name: &'static str,
    pub schema: SchemaRef,
    pub optional: bool,
}

pub struct Schema {
    pub kind: Kind,
    pub checks: Vec<Check>,
    pub description: Option<&'static str>,
    /// The published schema lists this description before the type. The
    /// original wrapped the field after describing it.
    pub description_first: bool,
}

impl Schema {
    pub const fn new(kind: Kind) -> Self {
        Schema { kind, checks: Vec::new(), description: None, description_first: false }
    }

    pub fn check(mut self, check: Check) -> Self {
        self.checks.push(check);
        self
    }

    pub fn refine(self, refine: fn(&Value, &mut Vec<Issue>)) -> Self {
        self.check(Check::Refine(refine))
    }

    pub fn describe(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    /// Describe a field whose published schema lists the description first.
    pub fn describe_first(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self.description_first = true;
        self
    }

    pub fn fields(&self) -> &[Field] {
        match &self.kind {
            Kind::Object { fields } => fields,
            _ => &[],
        }
    }

    /// Validate a value and return every issue in order.
    pub fn issues(&'static self, value: &Value) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut path = Vec::new();
        run(self, Some(value), &mut path, &mut issues);
        issues
    }

    /// The value a successful parse returns: object keys in published field
    /// order, record keys in input order, and nothing else changed. Call it
    /// only on a value that passed validation.
    pub fn output(&'static self, value: &Value) -> Value {
        match (&self.kind, value) {
            (Kind::Object { fields }, Value::Object(map)) => {
                let mut out = json::Map::new();
                for field in fields {
                    if let Some(child) = map.get(field.name) {
                        out.insert(field.name.to_string(), field.schema.output(child));
                    }
                }
                Value::Object(out)
            }
            (Kind::Array(items), Value::Array(elements)) => {
                Value::Array(elements.iter().map(|element| items.output(element)).collect())
            }
            (Kind::Record { value: value_schema, .. }, Value::Object(map)) => {
                Value::Object(map.iter().map(|(key, child)| (key.clone(), value_schema.output(child))).collect())
            }
            (Kind::Union(options), _) => match options.iter().find(|option| option.issues(value).is_empty()) {
                Some(option) => option.output(value),
                None => value.clone(),
            },
            (Kind::Discriminated { key, options }, Value::Object(map)) => {
                let tag = map.get(*key).and_then(Value::as_str);
                match options.iter().find(|(name, _)| Some(*name) == tag) {
                    Some((_, option)) => option.output(value),
                    None => value.clone(),
                }
            }
            (Kind::Nullable(inner), _) if !value.is_null() => inner.output(value),
            _ => value.clone(),
        }
    }

    pub fn validate(&'static self, value: &Value) -> Result<(), ValidationError> {
        let issues = self.issues(value);
        if issues.is_empty() {
            Ok(())
        } else {
            Err(ValidationError { issues })
        }
    }
}

/// A type with a published shape.
pub trait Shape {
    fn schema() -> SchemaRef;
}

impl<T: Shape> Shape for Vec<T> {
    fn schema() -> SchemaRef {
        // One static per element type is not expressible generically, so the
        // array node is leaked once per call site through a small cache.
        array_of(T::schema())
    }
}

/// Validate a JSON value against `T`'s shape and decode it.
pub fn decode<T>(value: &Value) -> Result<T, ValidationError>
where
    T: Shape + serde::de::DeserializeOwned,
{
    T::schema().validate(value)?;
    from_checked_value(value)
}

/// Decode a value that already passed validation.
pub fn from_checked_value<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, ValidationError> {
    let mut normalized = value.clone();
    normalize_numbers(&mut normalized);
    serde_json::from_value(normalized)
        .map_err(|error| ValidationError::single(Vec::new(), format!("Internal decode error: {error}")))
}

/// Rewrite integral doubles in the safe range as integers. JavaScript has one
/// number type; `5.0` and `5` are the same value and print the same way.
pub fn normalize_numbers(value: &mut Value) {
    match value {
        Value::Number(number) => {
            if number.is_f64() {
                let double = json::number_as_f64(number);
                if double.fract() == 0.0 && double.abs() <= MAX_SAFE_INTEGER {
                    let integer = double as i64;
                    *number = if integer >= 0 { (integer as u64).into() } else { integer.into() };
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_numbers),
        Value::Object(map) => map.values_mut().for_each(normalize_numbers),
        _ => {}
    }
}

static ARRAY_CACHE: LazyLock<std::sync::Mutex<Vec<(usize, SchemaRef)>>> =
    LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// The array shape for an element shape. Shapes are static, so each array node
/// is created once.
pub fn array_of(items: SchemaRef) -> SchemaRef {
    let key = items as *const Schema as usize;
    let mut cache = ARRAY_CACHE.lock().expect("schema cache lock");
    if let Some((_, found)) = cache.iter().find(|(address, _)| *address == key) {
        return found;
    }
    let created: SchemaRef = Box::leak(Box::new(Schema::new(Kind::Array(items))));
    cache.push((key, created));
    created
}

/// Allocate a shape for the life of the program.
pub fn leak(schema: Schema) -> SchemaRef {
    Box::leak(Box::new(schema))
}

fn received(value: Option<&Value>) -> &'static str {
    match value {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn push(issues: &mut Vec<Issue>, path: &[PathSegment], code: IssueCode, message: String) {
    issues.push(Issue { path: path.to_vec(), code, message });
}

fn invalid_type(issues: &mut Vec<Issue>, path: &[PathSegment], expected: &str, value: Option<&Value>) {
    push(
        issues,
        path,
        IssueCode::InvalidType,
        format!("Invalid input: expected {expected}, received {}", received(value)),
    );
}

fn quote_options(options: &[&str], separator: &str, quote: char) -> String {
    options.iter().map(|option| format!("{quote}{option}{quote}")).collect::<Vec<_>>().join(separator)
}

fn number_text(value: f64) -> String {
    json::format_number(value)
}

fn run(schema: &'static Schema, value: Option<&Value>, path: &mut Vec<PathSegment>, issues: &mut Vec<Issue>) {
    let start = issues.len();
    match &schema.kind {
        Kind::String => {
            let Some(Value::String(text)) = value else {
                invalid_type(issues, path, "string", value);
                // Length checks also run on any value with a length, so an
                // array reports its own length problem.
                if let Some(Value::Array(items)) = value {
                    run_array_length_checks(schema, items.len(), path, issues);
                }
                return;
            };
            run_string_checks(schema, text, path, issues);
        }
        Kind::Number { integer } => {
            let Some(Value::Number(number)) = value else {
                invalid_type(issues, path, "number", value);
                return;
            };
            let double = json::number_as_f64(number);
            if *integer {
                if double.fract() != 0.0 {
                    push(issues, path, IssueCode::InvalidType, "Invalid input: expected int, received number".into());
                    return;
                }
                if double > MAX_SAFE_INTEGER {
                    push(issues, path, IssueCode::TooBig, "Too big: expected int to be <=9007199254740991".into());
                } else if double < -MAX_SAFE_INTEGER {
                    push(issues, path, IssueCode::TooSmall, "Too small: expected int to be >=-9007199254740991".into());
                }
            }
            for check in &schema.checks {
                match check {
                    Check::Minimum { value: bound, inclusive } => {
                        let ok = if *inclusive { double >= *bound } else { double > *bound };
                        if !ok {
                            let relation = if *inclusive { ">=" } else { ">" };
                            push(
                                issues,
                                path,
                                IssueCode::TooSmall,
                                format!("Too small: expected number to be {relation}{}", number_text(*bound)),
                            );
                        }
                    }
                    Check::Maximum { value: bound, inclusive } => {
                        let ok = if *inclusive { double <= *bound } else { double < *bound };
                        if !ok {
                            let relation = if *inclusive { "<=" } else { "<" };
                            push(
                                issues,
                                path,
                                IssueCode::TooBig,
                                format!("Too big: expected number to be {relation}{}", number_text(*bound)),
                            );
                        }
                    }
                    Check::Refine(refine) => run_refine(*refine, value.unwrap(), path, issues, start),
                    _ => {}
                }
            }
        }
        Kind::Boolean => {
            if !matches!(value, Some(Value::Bool(_))) {
                invalid_type(issues, path, "boolean", value);
            }
        }
        Kind::Null => {
            if !matches!(value, Some(Value::Null)) {
                invalid_type(issues, path, "null", value);
            }
        }
        Kind::Enum(options) => {
            let matched = matches!(value, Some(Value::String(text)) if options.contains(&text.as_str()));
            if !matched {
                let message = if options.len() == 1 {
                    format!("Invalid input: expected \"{}\"", options[0])
                } else {
                    format!("Invalid option: expected one of {}", quote_options(options, "|", '"'))
                };
                push(issues, path, IssueCode::InvalidValue, message);
            }
        }
        Kind::Literal(expected) => {
            if !matches!(value, Some(Value::String(text)) if text == expected) {
                push(issues, path, IssueCode::InvalidValue, format!("Invalid input: expected \"{expected}\""));
            }
        }
        Kind::Object { fields } => {
            let Some(Value::Object(map)) = value else {
                invalid_type(issues, path, "object", value);
                return;
            };
            for field in fields {
                let child = map.get(field.name);
                if child.is_none() && field.optional {
                    continue;
                }
                path.push(PathSegment::Key(field.name.to_string()));
                run(field.schema, child, path, issues);
                path.pop();
            }
            let unknown: Vec<&str> = map
                .keys()
                .filter(|key| !fields.iter().any(|field| field.name == key.as_str()))
                .map(String::as_str)
                .collect();
            if !unknown.is_empty() {
                let message = if unknown.len() == 1 {
                    format!("Unrecognized key: \"{}\"", unknown[0])
                } else {
                    format!("Unrecognized keys: {}", quote_options(&unknown, ", ", '"'))
                };
                push(issues, path, IssueCode::UnrecognizedKeys, message);
            }
            for check in &schema.checks {
                if let Check::Refine(refine) = check {
                    run_refine(*refine, value.unwrap(), path, issues, start);
                }
            }
        }
        Kind::Array(items) => {
            let Some(Value::Array(elements)) = value else {
                invalid_type(issues, path, "array", value);
                return;
            };
            for (index, element) in elements.iter().enumerate() {
                path.push(PathSegment::Index(index));
                run(items, Some(element), path, issues);
                path.pop();
            }
            for check in &schema.checks {
                if aborted(issues, start) {
                    break;
                }
                match check {
                    Check::MaxItems(limit) if elements.len() > *limit => push(
                        issues,
                        path,
                        IssueCode::TooBig,
                        format!("Too big: expected array to have <={limit} items"),
                    ),
                    Check::Refine(refine) => run_refine(*refine, value.unwrap(), path, issues, start),
                    _ => {}
                }
            }
        }
        Kind::Record { key, value: value_schema } => {
            let Some(Value::Object(map)) = value else {
                invalid_type(issues, path, "record", value);
                return;
            };
            for (name, child) in map.iter() {
                let mut key_issues = Vec::new();
                let key_value = Value::String(name.clone());
                run(key, Some(&key_value), &mut Vec::new(), &mut key_issues);
                path.push(PathSegment::Key(name.clone()));
                if !key_issues.is_empty() {
                    push(issues, path, IssueCode::InvalidKey, "Invalid key in record".into());
                } else {
                    run(value_schema, Some(child), path, issues);
                }
                path.pop();
            }
            for check in &schema.checks {
                if let Check::Refine(refine) = check {
                    run_refine(*refine, value.unwrap(), path, issues, start);
                }
            }
        }
        Kind::Union(options) => run_union(options, value, path, issues),
        Kind::Discriminated { key, options } => {
            let Some(Value::Object(map)) = value else {
                invalid_type(issues, path, "object", value);
                return;
            };
            let tag = map.get(*key).and_then(Value::as_str);
            match options.iter().find(|(name, _)| Some(*name) == tag) {
                Some((_, option)) => run(option, value, path, issues),
                None => {
                    path.push(PathSegment::Key((*key).to_string()));
                    let names: Vec<&str> = options.iter().map(|(name, _)| *name).collect();
                    push(
                        issues,
                        path,
                        IssueCode::InvalidUnion,
                        format!("Invalid discriminator value. Expected {}", quote_options(&names, " | ", '\'')),
                    );
                    path.pop();
                }
            }
        }
        Kind::Nullable(inner) => {
            if !matches!(value, Some(Value::Null)) {
                run(inner, value, path, issues);
            }
        }
        Kind::Json => {
            if value.is_none() {
                push(issues, path, IssueCode::InvalidUnion, "Invalid input".into());
            }
        }
        Kind::ExtensionValue => run_union(crate::model::extensions::extension_value_options(), value, path, issues),
    }
}

/// A union passes when any option passes. When every option fails and exactly
/// one failed only with non-aborting issues, that option's issues are reported.
/// Otherwise the union reports one `invalid_union` issue.
fn run_union(options: &[SchemaRef], value: Option<&Value>, path: &mut Vec<PathSegment>, issues: &mut Vec<Issue>) {
    let mut attempts = Vec::with_capacity(options.len());
    for option in options {
        let mut attempt = Vec::new();
        run(option, value, path, &mut attempt);
        if attempt.is_empty() {
            return;
        }
        attempts.push(attempt);
    }
    let mut continuable = attempts.into_iter().filter(|attempt| !aborted(attempt, 0));
    if let (Some(only), None) = (continuable.next(), continuable.next()) {
        issues.extend(only);
        return;
    }
    push(issues, path, IssueCode::InvalidUnion, "Invalid input".into());
}

fn run_array_length_checks(schema: &'static Schema, length: usize, path: &[PathSegment], issues: &mut Vec<Issue>) {
    for check in &schema.checks {
        match check {
            Check::MinLength(minimum) if length < *minimum => {
                push(issues, path, IssueCode::TooSmall, format!("Too small: expected array to have >={minimum} items"))
            }
            Check::MaxLength(maximum) if length > *maximum => {
                push(issues, path, IssueCode::TooBig, format!("Too big: expected array to have <={maximum} items"))
            }
            _ => {}
        }
    }
}

fn run_string_checks(schema: &'static Schema, text: &str, path: &[PathSegment], issues: &mut Vec<Issue>) {
    let start = issues.len();
    for check in &schema.checks {
        match check {
            Check::MinLength(minimum) => {
                if text.chars().count() < *minimum {
                    push(
                        issues,
                        path,
                        IssueCode::TooSmall,
                        format!("Too small: expected string to have >={minimum} characters"),
                    );
                }
            }
            Check::MaxLength(maximum) => {
                if text.chars().count() > *maximum {
                    push(
                        issues,
                        path,
                        IssueCode::TooBig,
                        format!("Too big: expected string to have <={maximum} characters"),
                    );
                }
            }
            Check::Format(format) => {
                let (ok, message) = match format {
                    StringFormat::DateTimeMillisUtc => {
                        (crate::model::time::is_utc_timestamp(text), "Invalid ISO datetime")
                    }
                    StringFormat::Date => (crate::model::time::is_calendar_date(text), "Invalid ISO date"),
                };
                if !ok {
                    push(issues, path, IssueCode::InvalidFormat, message.into());
                }
            }
            Check::Pattern { test, message, .. } => {
                if !test(text) {
                    push(issues, path, IssueCode::InvalidFormat, (*message).into());
                }
            }
            Check::StringRefine { test, message } => {
                if !test(text) {
                    push(issues, path, IssueCode::Custom, (*message).into());
                }
            }
            Check::Refine(refine) => {
                run_refine(*refine, &Value::String(text.to_string()), path, issues, start);
            }
            _ => {}
        }
    }
}

fn aborted(issues: &[Issue], start: usize) -> bool {
    issues[start..].iter().any(|issue| issue.code.aborts())
}

fn run_refine(
    refine: fn(&Value, &mut Vec<Issue>),
    value: &Value,
    path: &[PathSegment],
    issues: &mut Vec<Issue>,
    start: usize,
) {
    if aborted(issues, start) {
        return;
    }
    let mut found = Vec::new();
    refine(value, &mut found);
    for mut issue in found {
        let mut full = path.to_vec();
        full.append(&mut issue.path);
        issue.path = full;
        issues.push(issue);
    }
}

/// Add a custom issue from a refinement.
pub fn custom(issues: &mut Vec<Issue>, path: &[&str], message: impl Into<String>) {
    issues.push(Issue {
        path: path.iter().map(|segment| PathSegment::Key((*segment).to_string())).collect(),
        code: IssueCode::Custom,
        message: message.into(),
    });
}

// ---- Shape constructors -------------------------------------------------

pub fn string() -> Schema {
    Schema::new(Kind::String)
}

pub fn number() -> Schema {
    Schema::new(Kind::Number { integer: false })
}

pub fn integer() -> Schema {
    Schema::new(Kind::Number { integer: true })
}

pub fn min(value: f64) -> Check {
    Check::Minimum { value, inclusive: true }
}

pub fn max(value: f64) -> Check {
    Check::Maximum { value, inclusive: true }
}

pub fn above(value: f64) -> Check {
    Check::Minimum { value, inclusive: false }
}

macro_rules! static_schema {
    ($name:ident, $build:expr) => {
        pub fn $name() -> SchemaRef {
            static SCHEMA: LazyLock<Schema> = LazyLock::new(|| $build);
            &SCHEMA
        }
    };
}

static_schema!(text, string().check(Check::MinLength(1)));
static_schema!(any_string, string());
static_schema!(any_number, number());
static_schema!(positive, number().check(above(0.0)));
static_schema!(nonnegative, number().check(min(0.0)));
static_schema!(positive_int, integer().check(above(0.0)));
static_schema!(nonnegative_int, integer().check(min(0.0)));
static_schema!(boolean, Schema::new(Kind::Boolean));
static_schema!(json_value, Schema::new(Kind::Json));
static_schema!(supplied_fields, Schema::new(Kind::Record { key: text(), value: json_value() }));

/// A text value with the given literal.
pub fn literal(value: &'static str) -> SchemaRef {
    leak(Schema::new(Kind::Literal(value)))
}

impl Shape for String {
    fn schema() -> SchemaRef {
        text()
    }
}

impl Shape for bool {
    fn schema() -> SchemaRef {
        boolean()
    }
}

impl Shape for f64 {
    fn schema() -> SchemaRef {
        any_number()
    }
}

/// Declare a model object: one Rust struct and its published shape.
///
/// ```ignore
/// model_object! {
///     pub struct Example {
///         name: String,                       // required, shape from `Shape`
///         note?: String,                      // optional, never `null`
///         count?: u64 = nonnegative_int(),    // explicit shape
///     }
///     refine(check_example)
/// }
/// ```
#[macro_export]
macro_rules! model_object {
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($body:tt)* }
        $(refine($($refine:path),* $(,)?))?
        $(describe($description:expr))?
    ) => {
        $crate::model_object!(@munch
            [$(#[$meta])*] $name [$($($refine),*)?] [$($description)?] [] [] $($body)*
        );
    };

    (@munch $meta:tt $name:ident $refine:tt $description:tt [$($fields:tt)*] [$($shape:tt)*]
        $(#[$field_meta:meta])* $field:ident ? : $ty:ty = $schema:expr $(, $($rest:tt)*)?
    ) => {
        $crate::model_object!(@munch $meta $name $refine $description
            [$($fields)* $(#[$field_meta])* #[serde(default, skip_serializing_if = "Option::is_none")] pub $field: Option<$ty>,]
            [$($shape)* $crate::model::schema::Field { name: stringify!($field), schema: $schema, optional: true },]
            $($($rest)*)?
        );
    };

    (@munch $meta:tt $name:ident $refine:tt $description:tt [$($fields:tt)*] [$($shape:tt)*]
        $(#[$field_meta:meta])* $field:ident ? : $ty:ty $(, $($rest:tt)*)?
    ) => {
        $crate::model_object!(@munch $meta $name $refine $description
            [$($fields)* $(#[$field_meta])* #[serde(default, skip_serializing_if = "Option::is_none")] pub $field: Option<$ty>,]
            [$($shape)* $crate::model::schema::Field { name: stringify!($field), schema: <$ty as $crate::model::schema::Shape>::schema(), optional: true },]
            $($($rest)*)?
        );
    };

    (@munch $meta:tt $name:ident $refine:tt $description:tt [$($fields:tt)*] [$($shape:tt)*]
        $(#[$field_meta:meta])* $field:ident : $ty:ty = $schema:expr $(, $($rest:tt)*)?
    ) => {
        $crate::model_object!(@munch $meta $name $refine $description
            [$($fields)* $(#[$field_meta])* pub $field: $ty,]
            [$($shape)* $crate::model::schema::Field { name: stringify!($field), schema: $schema, optional: false },]
            $($($rest)*)?
        );
    };

    (@munch $meta:tt $name:ident $refine:tt $description:tt [$($fields:tt)*] [$($shape:tt)*]
        $(#[$field_meta:meta])* $field:ident : $ty:ty $(, $($rest:tt)*)?
    ) => {
        $crate::model_object!(@munch $meta $name $refine $description
            [$($fields)* $(#[$field_meta])* pub $field: $ty,]
            [$($shape)* $crate::model::schema::Field { name: stringify!($field), schema: <$ty as $crate::model::schema::Shape>::schema(), optional: false },]
            $($($rest)*)?
        );
    };

    (@munch [$($meta:tt)*] $name:ident [$($refine:path),*] [$($description:expr)?] [$($fields:tt)*] [$($shape:tt)*]) => {
        $($meta)*
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $($fields)* }

        impl $crate::model::schema::Shape for $name {
            fn schema() -> $crate::model::schema::SchemaRef {
                static SCHEMA: std::sync::LazyLock<$crate::model::schema::Schema> = std::sync::LazyLock::new(|| {
                    #[allow(unused_mut)]
                    let mut schema = $crate::model::schema::Schema::new($crate::model::schema::Kind::Object {
                        fields: vec![$($shape)*],
                    });
                    $( schema = schema.refine($refine); )*
                    $( schema = schema.describe($description); )?
                    schema
                });
                &SCHEMA
            }
        }
    };
}

/// Declare a string enum with its published values.
#[macro_export]
macro_rules! model_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $text:literal),* $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        pub enum $name {
            $($(#[$variant_meta])* #[serde(rename = $text)] $variant),*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];
            pub const NAMES: &'static [&'static str] = &[$($text),*];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),* }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text { $($text => Some($name::$variant),)* _ => None }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl $crate::model::schema::Shape for $name {
            fn schema() -> $crate::model::schema::SchemaRef {
                static SCHEMA: std::sync::LazyLock<$crate::model::schema::Schema> = std::sync::LazyLock::new(|| {
                    $crate::model::schema::Schema::new($crate::model::schema::Kind::Enum($name::NAMES))
                });
                &SCHEMA
            }
        }
    };
}
