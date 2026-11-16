//! Shapes shared by several record types.

use std::sync::LazyLock;

use super::schema::{
    custom, leak, literal, nonnegative, positive, positive_int, string, text, Check, Field, Issue, Kind, Schema,
    SchemaRef, Shape, StringFormat,
};
use super::time;
use crate::json::{Map, Value};
use crate::{model_enum, model_object};

// ---- Checked strings ----------------------------------------------------

pub const RECORD_ID_PATTERN: &str = "^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$";
pub const SEMVER_PATTERN: &str = r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$";
pub const SHA256_PATTERN: &str = "^[a-f0-9]{64}$";
pub const BENCHMARK_KEY_PATTERN: &str = "^[a-z0-9][a-z0-9_-]*$";
pub const PACKAGE_RELATIVE_PATH_PATTERN: &str = r"^(?!\/)(?!.*\\)(?!.*(?:^|\/)\.\.(?:\/|$)).+$";

/// A lowercase RFC 9562 version 7 UUID.
pub fn is_record_id(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let ok = match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            14 => *byte == b'7',
            19 => matches!(byte, b'8' | b'9' | b'a' | b'b'),
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        };
        if !ok {
            return false;
        }
    }
    true
}

pub fn is_semver(text: &str) -> bool {
    let (core, rest) = match text.find(['-', '+']) {
        Some(index) => (&text[..index], &text[index..]),
        None => (text, ""),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    let identifier =
        |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if rest.is_empty() {
        return true;
    }
    if let Some(after) = rest.strip_prefix('-') {
        match after.find('+') {
            Some(plus) => identifier(&after[..plus]) && identifier(&after[plus + 1..]),
            None => identifier(after),
        }
    } else if let Some(build) = rest.strip_prefix('+') {
        identifier(build)
    } else {
        false
    }
}

pub fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_benchmark_key(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes[1..].iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

/// A relative package path: not absolute, no backslash, no `..` segment, and
/// not empty. Line breaks fail as they do in the published pattern, where `.`
/// does not match a line terminator.
pub fn is_package_relative_path(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('/')
        && !text.contains('\\')
        && !text.split('/').any(|segment| segment == "..")
        && !text.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

/// A timezone the published validator accepts: an IANA name matched without
/// regard to case, or a fixed UTC offset such as `+05:00`, `+0530`, or `-08`.
pub fn is_iana_time_zone(text: &str) -> bool {
    resolve_time_zone(text).is_some() || fixed_offset_minutes(text).is_some()
}

/// Minutes east of UTC for a fixed offset written `+HH`, `+HHMM`, or `+HH:MM`.
pub fn fixed_offset_minutes(text: &str) -> Option<i32> {
    let bytes = text.as_bytes();
    let sign = match bytes.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = |range: std::ops::Range<usize>| -> Option<i32> {
        let part = text.get(range)?;
        (part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse().ok()).flatten()
    };
    let (hours, minutes) = match bytes.len() {
        3 => (digits(1..3)?, 0),
        5 => (digits(1..3)?, digits(3..5)?),
        6 if bytes[3] == b':' => (digits(1..3)?, digits(4..6)?),
        _ => return None,
    };
    (hours <= 23 && minutes <= 59).then_some(sign * (hours * 60 + minutes))
}

/// The timezone for an IANA name, matched without regard to case.
pub fn resolve_time_zone(text: &str) -> Option<chrono_tz::Tz> {
    if text.is_empty() {
        return None;
    }
    text.parse::<chrono_tz::Tz>().ok().or_else(|| chrono_tz::Tz::from_str_insensitive(text).ok())
}

macro_rules! schema_fn {
    ($(#[$meta:meta])* $name:ident, $build:expr) => {
        $(#[$meta])*
        pub fn $name() -> SchemaRef {
            static SCHEMA: LazyLock<Schema> = LazyLock::new(|| $build);
            &SCHEMA
        }
    };
}

schema_fn!(
    /// Stable UUIDv7 record ID.
    record_id,
    string()
        .check(Check::Pattern { source: RECORD_ID_PATTERN, test: is_record_id, message: "lowercase UUIDv7" })
        .describe("Stable UUIDv7 record ID")
);
schema_fn!(
    database_id,
    string()
        .check(Check::Pattern { source: RECORD_ID_PATTERN, test: is_record_id, message: "lowercase UUIDv7" })
        .describe("Stable UUIDv7 database ID")
);
schema_fn!(
    utc_timestamp,
    string()
        .check(Check::Format(StringFormat::DateTimeMillisUtc))
        .describe("Normalized ISO 8601 timestamp in UTC with millisecond precision")
);
schema_fn!(calendar_date, string().check(Check::Format(StringFormat::Date)).describe("ISO 8601 calendar date"));
schema_fn!(
    /// A namespaced extension ID such as `org.example/widget`.
    extension_id,
    string()
        .check(Check::Pattern {
            source: super::extensions::EXTENSION_ID_PATTERN,
            test: super::extensions::is_extension_id_pattern,
            message: "namespaced extension id",
        })
        .check(Check::MaxLength(200))
);
schema_fn!(
    semver,
    string().check(Check::Pattern { source: SEMVER_PATTERN, test: is_semver, message: "semantic version" })
);
schema_fn!(
    sha256,
    string().check(Check::Pattern { source: SHA256_PATTERN, test: is_sha256, message: "lowercase SHA-256 hex digest" })
);
schema_fn!(
    benchmark_key,
    string().check(Check::Pattern {
        source: BENCHMARK_KEY_PATTERN,
        test: is_benchmark_key,
        message: "lowercase key with hyphens or underscores",
    })
);
schema_fn!(
    package_relative_path,
    string().check(Check::MinLength(1)).check(Check::Pattern {
        source: PACKAGE_RELATIVE_PATH_PATTERN,
        test: is_package_relative_path,
        message: "Package path must be relative and cannot traverse parent directories",
    })
);
schema_fn!(
    iana_time_zone,
    string()
        .check(Check::MinLength(1))
        .check(Check::StringRefine { test: is_iana_time_zone, message: "IANA timezone" })
        .describe_first("IANA timezone for calendar calculations")
);
schema_fn!(revision_number, positive_int_schema());
schema_fn!(previous_revision, Schema::new(Kind::Nullable(positive_int())));
schema_fn!(sleep_stage_minutes, Schema::new(Kind::Record { key: text(), value: nonnegative() }));
schema_fn!(text_array, Schema::new(Kind::Array(text())));
schema_fn!(record_id_array, Schema::new(Kind::Array(record_id())));

fn positive_int_schema() -> Schema {
    super::schema::integer().check(super::schema::above(0.0))
}

/// A value kept under the name it arrived with (`supplied_fields`).
pub type SuppliedFields = Map<String, Value>;

/// Stage name to minutes.
pub type StageMinutes = Map<String, Value>;

// ---- Enums --------------------------------------------------------------

model_enum! {
    pub enum RecordType {
        AthleteProfile = "athlete_profile",
        Source = "source",
        SourceDevice = "source_device",
        PointMeasurement = "point_measurement",
        MeasurementSample = "measurement_sample",
        SleepSession = "sleep_session",
        WorkoutSession = "workout_session",
        BenchmarkResult = "benchmark_result",
        ProviderSummary = "provider_summary",
        ProviderScore = "provider_score",
        AthleteReport = "athlete_report",
        NutritionEntry = "nutrition_entry",
        Attachment = "attachment",
        BenchmarkDefinition = "benchmark_definition",
        Prediction = "prediction",
        PredictionResultLink = "prediction_result_link",
        PredictionGrade = "prediction_grade",
        PredictionErrorAnalysis = "prediction_error_analysis",
        TrainingPlan = "training_plan",
        PlannedSession = "planned_session",
        CoachingNote = "coaching_note",
        SavedTrainingReview = "saved_training_review",
        Goal = "goal",
        NutritionModelInterpretation = "nutrition_model_interpretation",
    }
}

model_enum! {
    pub enum AuthoredRecordStatus {
        Draft = "draft",
        Active = "active",
        Completed = "completed",
        Superseded = "superseded",
        Cancelled = "cancelled",
    }
}

model_enum! {
    pub enum MissingReason {
        NotProvidedBySource = "not_provided_by_source",
        NotExposedByRoute = "not_exposed_by_route",
        NotApplicable = "not_applicable",
    }
}

model_enum! {
    pub enum InputTimestampBasis {
        SourceInstant = "source_instant",
        SourceOffsetTimestamp = "source_offset_timestamp",
        SourceLocalTimestampWithDocumentedOffset = "source_local_timestamp_with_documented_offset",
        ReconstructedFromSeriesStart = "reconstructed_from_series_start",
    }
}

model_enum! {
    pub enum UtcOffsetMeaning {
        DocumentedRecordingOffset = "documented_recording_offset",
        ProviderCycleOffset = "provider_cycle_offset",
        RouteRenderingOffset = "route_rendering_offset",
        MeaningNotDocumented = "meaning_not_documented",
    }
}

model_enum! {
    pub enum SourceTimeZoneMeaning {
        AppleHealthObjectCreationTimeZone = "apple_health_object_creation_time_zone",
    }
}

model_enum! {
    pub enum ReceivedAtBasis {
        ProviderSupplied = "provider_supplied",
        ConnectorReceivedTime = "connector_received_time",
        FileModifiedTime = "file_modified_time",
        ImportTime = "import_time",
        LocalWriteTime = "local_write_time",
    }
}

model_enum! {
    pub enum ProviderReportingPeriodType {
        WhoopPhysiologicalCycle = "whoop_physiological_cycle",
        OuraActivityDay = "oura_activity_day",
        OuraSleepDay = "oura_sleep_day",
        OuraCalendarDay = "oura_calendar_day",
        OuraDayUnspecified = "oura_day_unspecified",
        AppleActivitySummaryDay = "apple_activity_summary_day",
    }
}

model_enum! {
    /// Bases for a stored value (D185, D187, D192).
    pub enum ValueBasis {
        ProviderOrAggregator = "provider_or_aggregator",
        Provider = "provider",
        HrvMeanRmssd = "hrv_mean_rmssd",
        ProviderDocumentedRmssd = "provider_documented_rmssd",
        CalculatedOnImport = "calculated_on_import",
        EnteredByHand = "entered_by_hand",
    }
}

model_enum! {
    /// Where a workout's activity type came from (D185, D187, D192).
    pub enum ActivityTypeBasis {
        AggregatorNormalized = "aggregator_normalized",
        ProviderLabel = "provider_label",
    }
}

model_enum! {
    pub enum RouteSummariesBasis {
        CalculatedOnImport = "calculated_on_import",
    }
}

model_enum! {
    pub enum BecameKnownAtBasis {
        LocalWrite = "local_write",
        ProviderImport = "provider_import",
        PackageRestore = "package_restore",
        PackageMerge = "package_merge",
    }
}

model_enum! {
    pub enum ErasureReasonCode {
        AthleteRequest = "athlete_request",
        LegalRequirement = "legal_requirement",
        RecordedInError = "recorded_in_error",
        Other = "other",
    }
}

// ---- Evidence and actors ------------------------------------------------

model_object! {
    /// A record and the exact revision used.
    pub struct EvidenceReference {
        record_id: String = record_id(),
        revision: u64 = revision_number(),
    }
}

/// Who made a change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "author_type", rename_all = "snake_case")]
pub enum RecordActor {
    Athlete,
    Agent {
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_version: Option<String>,
    },
    Tool {
        tool: String,
        tool_version: String,
    },
}

impl RecordActor {
    pub fn author_type(&self) -> &'static str {
        match self {
            RecordActor::Athlete => "athlete",
            RecordActor::Agent { .. } => "agent",
            RecordActor::Tool { .. } => "tool",
        }
    }
}

/// The author of an authored record.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "author_type", rename_all = "snake_case")]
pub enum AuthoredRecordAuthor {
    Athlete,
    Agent {
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_version: Option<String>,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_version: Option<String>,
    },
    Tool {
        tool: String,
        tool_version: String,
        calculation_version: String,
    },
}

impl AuthoredRecordAuthor {
    pub fn author_type(&self) -> &'static str {
        match self {
            AuthoredRecordAuthor::Athlete => "athlete",
            AuthoredRecordAuthor::Agent { .. } => "agent",
            AuthoredRecordAuthor::Tool { .. } => "tool",
        }
    }
}

fn field(name: &'static str, schema: SchemaRef, optional: bool) -> Field {
    Field { name, schema, optional }
}

fn athlete_actor_fields() -> Vec<Field> {
    vec![field("author_type", literal("athlete"), false)]
}

fn agent_actor_fields() -> Vec<Field> {
    vec![
        field("author_type", literal("agent"), false),
        field("agent", text(), false),
        field("agent_version", semver(), true),
    ]
}

fn tool_actor_fields() -> Vec<Field> {
    vec![
        field("author_type", literal("tool"), false),
        field("tool", text(), false),
        field("tool_version", semver(), false),
    ]
}

/// The athlete actor shape.
pub fn athlete_actor() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> =
        LazyLock::new(|| leak(Schema::new(Kind::Object { fields: athlete_actor_fields() })));
    &SCHEMA
}

/// The agent actor shape.
pub fn agent_actor() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> =
        LazyLock::new(|| leak(Schema::new(Kind::Object { fields: agent_actor_fields() })));
    &SCHEMA
}

/// The tool actor shape.
pub fn tool_actor() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> =
        LazyLock::new(|| leak(Schema::new(Kind::Object { fields: tool_actor_fields() })));
    &SCHEMA
}

/// The agent author shape: an agent actor with its model.
pub fn agent_author() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> = LazyLock::new(|| {
        let mut fields = agent_actor_fields();
        fields.push(field("model", text(), false));
        fields.push(field("model_version", text(), true));
        leak(Schema::new(Kind::Object { fields }))
    });
    &SCHEMA
}

/// The tool author shape: a tool actor with its calculation version.
pub fn tool_author() -> SchemaRef {
    static SCHEMA: LazyLock<&'static Schema> = LazyLock::new(|| {
        let mut fields = tool_actor_fields();
        fields.push(field("calculation_version", text(), false));
        leak(Schema::new(Kind::Object { fields }))
    });
    &SCHEMA
}

impl Shape for RecordActor {
    fn schema() -> SchemaRef {
        static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
            Schema::new(Kind::Discriminated {
                key: "author_type",
                options: vec![("athlete", athlete_actor()), ("agent", agent_actor()), ("tool", tool_actor())],
            })
        });
        &SCHEMA
    }
}

impl Shape for AuthoredRecordAuthor {
    fn schema() -> SchemaRef {
        static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
            Schema::new(Kind::Discriminated {
                key: "author_type",
                options: vec![("athlete", athlete_actor()), ("agent", agent_author()), ("tool", tool_author())],
            })
        });
        &SCHEMA
    }
}

model_object! {
    /// Who wrote an authored record, when, from what evidence, and its status.
    pub struct AuthoredRecordMetadata {
        author: AuthoredRecordAuthor,
        authored_at: String = utc_timestamp(),
        evidence: Vec<EvidenceReference>,
        content_version: u64 = positive_int(),
        status: AuthoredRecordStatus,
    }
}

// ---- Time receipts ------------------------------------------------------

model_object! {
    /// The time text a source wrote, and how it was read.
    pub struct InputTimestampReceipt {
        timestamp_text?: String,
        utc_offset_text?: String,
        utc_offset_meaning?: UtcOffsetMeaning,
        source_time_zone?: String,
        source_time_zone_meaning?: SourceTimeZoneMeaning,
        timestamp_basis?: InputTimestampBasis,
        timestamp_missing_reason?: MissingReason,
        utc_offset_missing_reason?: MissingReason,
    }
    refine(refine_input_timestamp)
}

fn has(value: &Value, key: &str) -> bool {
    value.get(key).is_some()
}

fn refine_input_timestamp(value: &Value, issues: &mut Vec<Issue>) {
    let timestamp = has(value, "timestamp_text");
    let timestamp_missing = has(value, "timestamp_missing_reason");
    let offset = has(value, "utc_offset_text");
    let offset_missing = has(value, "utc_offset_missing_reason");
    if !timestamp && !timestamp_missing {
        custom(issues, &["timestamp_missing_reason"], "A missing input timestamp needs a reason");
    }
    if timestamp && timestamp_missing {
        custom(issues, &["timestamp_missing_reason"], "An available input timestamp cannot also be marked missing");
    }
    if timestamp && !has(value, "timestamp_basis") {
        custom(issues, &["timestamp_basis"], "An input timestamp needs its documented basis");
    }
    if !offset && !offset_missing {
        custom(issues, &["utc_offset_missing_reason"], "A missing input UTC offset needs a reason");
    }
    if offset && offset_missing {
        custom(issues, &["utc_offset_missing_reason"], "An available input UTC offset cannot also be marked missing");
    }
    if offset && !has(value, "utc_offset_meaning") {
        custom(issues, &["utc_offset_meaning"], "An input UTC offset needs its documented meaning");
    }
    if !offset && has(value, "utc_offset_meaning") {
        custom(issues, &["utc_offset_meaning"], "UTC offset meaning requires UTC offset text");
    }
    if has(value, "source_time_zone") != has(value, "source_time_zone_meaning") {
        custom(
            issues,
            &["source_time_zone_meaning"],
            "A source timezone and its documented meaning must be provided together",
        );
    }
}

model_object! {
    /// A period a provider assigned to a value, such as a WHOOP cycle or an Oura day.
    pub struct ProviderReportingPeriod {
        period_type: ProviderReportingPeriodType,
        provider_period_id?: String,
        provider_date?: String = calendar_date(),
        start_at?: String = utc_timestamp(),
        end_at?: String = utc_timestamp(),
        input_start_timestamp?: InputTimestampReceipt,
        input_end_timestamp?: InputTimestampReceipt,
        timezone_offset_text?: String,
        timezone_offset_meaning?: UtcOffsetMeaning,
        timezone_offset_missing_reason?: MissingReason,
    }
    refine(refine_reporting_period)
}

/// `Date.parse` for the timestamp forms that reach refinements. Stored
/// timestamps use one exact form. Other ISO 8601 forms are read as JavaScript
/// reads them so a refinement on invalid input reports the same issues.
pub fn js_date_parse(text: &str) -> Option<i64> {
    if let Some(millis) = time::parse_utc_timestamp(text) {
        return Some(millis);
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(parsed.timestamp_millis());
    }
    if let Some((year, month, day)) = time::parse_calendar_date(text) {
        return Some(time::days_from_civil(year as i64, month, day) * 86_400_000);
    }
    None
}

fn date_value(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_str).and_then(js_date_parse)
}

fn refine_reporting_period(value: &Value, issues: &mut Vec<Issue>) {
    if !has(value, "provider_period_id") && !has(value, "provider_date") && !has(value, "start_at") {
        custom(issues, &[], "A provider reporting period needs an ID, date, or start instant");
    }
    if let (Some(start), Some(end)) = (date_value(value, "start_at"), date_value(value, "end_at")) {
        if end < start {
            custom(issues, &["end_at"], "Provider reporting period end cannot precede its start");
        }
    }
    if has(value, "timezone_offset_text") && !has(value, "timezone_offset_meaning") {
        custom(issues, &["timezone_offset_meaning"], "A provider UTC offset needs its documented meaning");
    }
    if has(value, "timezone_offset_text") && has(value, "timezone_offset_missing_reason") {
        custom(
            issues,
            &["timezone_offset_missing_reason"],
            "An available provider UTC offset cannot also be marked missing",
        );
    }
}

/// `end >= start` as `Date.parse` compares them. Unreadable text fails.
pub fn end_not_before_start(value: &Value, start: &str, end: &str) -> bool {
    match (date_value(value, start), date_value(value, end)) {
        (Some(start), Some(end)) => end >= start,
        _ => false,
    }
}

// ---- Source values ------------------------------------------------------

model_object! {
    /// A measured value in its standard unit, with the value and unit the source wrote.
    pub struct SourceValue {
        standard_value: f64,
        standard_unit: String,
        input_value?: f64,
        input_unit?: String,
        input_value_missing_reason?: MissingReason,
        input_unit_missing_reason?: MissingReason,
    }
    refine(refine_source_value)
}

fn refine_source_value(value: &Value, issues: &mut Vec<Issue>) {
    if has(value, "input_value") == has(value, "input_value_missing_reason") {
        custom(issues, &["input_value_missing_reason"], "Provide the input value or one missing reason");
    }
    if has(value, "input_unit") == has(value, "input_unit_missing_reason") {
        custom(issues, &["input_unit_missing_reason"], "Provide the input unit or one missing reason");
    }
}

model_object! {
    /// A summary value a source reported, such as a total distance.
    pub struct SourceReportedSummary {
        standard_value: f64,
        standard_unit: String,
        input_value?: f64,
        input_unit?: String,
        input_value_missing_reason?: MissingReason,
        input_unit_missing_reason?: MissingReason,
        summary_type: String,
    }
    refine(refine_source_value)
}

model_object! {
    /// The basis this program assigns to a stored value. It is not a supplied field (D185, D187).
    pub struct FieldBasis {
        activity_type?: ActivityTypeBasis,
        value?: ValueBasis,
        route_summaries?: RouteSummariesBasis = literal("calculated_on_import"),
    }
}

// ---- Scores -------------------------------------------------------------

model_object! {
    /// A benchmark score: time, repetitions, or load.
    pub struct Score {
        duration_s?: f64 = positive(),
        reps?: u64 = positive_int(),
        weight_kg?: f64 = positive(),
    }
    refine(refine_score)
}

fn refine_score(value: &Value, issues: &mut Vec<Issue>) {
    if !has(value, "duration_s") && !has(value, "reps") && !has(value, "weight_kg") {
        custom(issues, &[], "A score needs duration_s, reps, or weight_kg");
    }
}

impl Score {
    /// The value for a benchmark score type: `time`, `reps`, or `load`.
    pub fn value_for(&self, score_type: &str) -> Option<f64> {
        match score_type {
            "time" => self.duration_s,
            "reps" => self.reps.map(|reps| reps as f64),
            "load" => self.weight_kg,
            _ => None,
        }
    }
}

model_enum! {
    pub enum Confidence {
        Low = "low",
        Moderate = "moderate",
        High = "high",
    }
}
