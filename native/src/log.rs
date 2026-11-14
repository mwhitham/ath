//! `ath log`: one plain-text entry or one validated structured batch.
//!
//! Plain text becomes one record: a manual measurement, a benchmark result, a
//! rated athlete report, or a note. A JSON object or array is a structured
//! batch that is checked and written as one unit.
//!
//! Manual values stay visibly manual. A measurement typed here is written
//! under the manual source with `field_basis.value: entered_by_hand` (D187),
//! and device queries and baselines leave manual sources out. "HRV" alone is
//! refused, so RMSSD and SDNN never share a baseline.
//!
//! When a new result has exactly one open prediction for its benchmark, the
//! batch also links them and writes the deterministic grade.
//!
//! Attachments are prepared by the package module. `add_attachments` puts the
//! prepared attachment records into the batch and links them to its one
//! athlete report or nutrition entry. When attachment files must be written,
//! the package module writes them around the database change from
//! `LogPlan::changes`; without files, `apply_log_plan` writes the batch.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::json::{self, Map, Value};
use crate::model::grade::grade_arithmetic;
use crate::model::*;
use crate::prediction::grade::DEFAULT_GRADE_CALCULATION_VERSION;
use crate::store::query::parse_utc_timestamp;
use crate::store::{Applied, Changes, NewRecord, Store};

/// What to log.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRequest {
    /// The entry text, or a JSON object or array.
    pub text: String,
    pub actor: RecordActor,
    /// The exact UTC event and write time. Now by default.
    pub created_at: Option<String>,
    /// Names the workout for a plain-text result.
    pub benchmark: Option<String>,
}

/// A record the batch will create.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRecord {
    pub record_id: String,
    pub created_at: String,
    pub changed_by: RecordActor,
    pub data: RecordData,
    pub extensions: Option<Extensions>,
}

impl LogRecord {
    /// An attachment record prepared by the package module.
    pub fn attachment(record_id: String, created_at: String, changed_by: RecordActor, data: AttachmentData) -> Self {
        LogRecord { record_id, created_at, changed_by, data: RecordData::Attachment(data), extensions: None }
    }

    pub fn record_type(&self) -> RecordType {
        self.data.record_type()
    }

    fn to_new_record(&self) -> NewRecord {
        let mut record = NewRecord::new(self.data.clone(), self.changed_by.clone());
        record.record_id = Some(self.record_id.clone());
        record.created_at = Some(self.created_at.clone());
        record.extensions = self.extensions.clone();
        record
    }
}

/// A checked log batch, ready to preview and write.
#[derive(Debug, Clone, PartialEq)]
pub struct LogPlan {
    pub inputs: Vec<LogRecord>,
    /// Records that must still be at these revisions when the batch is written.
    pub expected_current_revisions: Vec<(String, u64)>,
    /// How the entry was understood, such as `athlete note`.
    pub interpretation: String,
}

/// One record in a log preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogPreviewRecord {
    pub record_id: String,
    pub record_type: RecordType,
    pub created_at: String,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Extensions>,
}

/// The `ath log --json` preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogPreview {
    pub file: String,
    pub interpretation: String,
    pub records: Vec<LogPreviewRecord>,
    /// Always false: a preview never writes.
    pub write_confirmed: bool,
}

impl LogPlan {
    pub fn preview(&self, file: &str) -> LogPreview {
        LogPreview {
            file: file.to_string(),
            interpretation: self.interpretation.clone(),
            records: self
                .inputs
                .iter()
                .map(|input| LogPreviewRecord {
                    record_id: input.record_id.clone(),
                    record_type: input.record_type(),
                    created_at: input.created_at.clone(),
                    data: input.data.to_value(),
                    extensions: input.extensions.clone(),
                })
                .collect(),
            write_confirmed: false,
        }
    }

    /// The complete database change for this batch.
    pub fn changes(&self) -> Changes {
        Changes {
            expected_current_revisions: self.expected_current_revisions.clone(),
            creates: self.inputs.iter().map(LogRecord::to_new_record).collect(),
            ..Changes::default()
        }
    }
}

/// Write a batch that has no attachment files, in one transaction.
pub fn apply_log_plan(store: &mut Store, plan: &LogPlan) -> Result<Applied> {
    store.apply(plan.changes())
}

/// The human preview `ath log` printed.
pub fn render_log_plan(plan: &LogPlan) -> String {
    let mut lines =
        vec![format!("understood: {}", plan.interpretation), format!("records to write: {}", plan.inputs.len())];
    let indented = |value: &Value, lines: &mut Vec<String>| {
        for line in json::to_pretty(value).split('\n') {
            lines.push(format!("    {line}"));
        }
    };
    for input in &plan.inputs {
        lines.push(format!("  {}  {}", input.record_id, input.record_type()));
        indented(&input.data.to_value(), &mut lines);
        if let Some(extensions) = &input.extensions {
            lines.push("    extensions".into());
            indented(&Value::Object(extensions.clone()), &mut lines);
        }
    }
    lines.join("\n")
}

/// Plan a log entry. Reads the database; writes nothing.
pub fn plan_log_entry(
    connection: &Connection,
    request: &LogRequest,
    new_id: &mut dyn FnMut() -> String,
) -> Result<LogPlan> {
    let created_at = parse_utc_timestamp(&request.created_at.clone().unwrap_or_else(time::now_utc))?;
    let trimmed = js_trim(&request.text);
    let plan = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let value = json::parse(trimmed)
            .map_err(|error| Error::other(format!("structured log is not valid JSON: {}", error.message)))?;
        plan_structured(&value, &request.actor, &created_at, new_id)?
    } else {
        plan_plain_text(connection, trimmed, &request.actor, &created_at, request.benchmark.as_deref(), new_id)?
    };
    add_automatic_grades(connection, plan, &request.actor, &created_at, new_id)
}

/// Put prepared attachment records into a batch and link them to its one
/// athlete report or nutrition entry.
pub fn add_attachments(plan: LogPlan, attachments: Vec<LogRecord>) -> Result<LogPlan> {
    if attachments.is_empty() {
        return Ok(plan);
    }
    let linkable: Vec<usize> = plan
        .inputs
        .iter()
        .enumerate()
        .filter(|(_, input)| matches!(input.record_type(), RecordType::AthleteReport | RecordType::NutritionEntry))
        .map(|(index, _)| index)
        .collect();
    let [index] = linkable[..] else {
        return Err(Error::refused("attachments need exactly one athlete_report or nutrition_entry in the log batch"));
    };
    let ids: Vec<String> = attachments.iter().map(|attachment| attachment.record_id.clone()).collect();
    let mut inputs = plan.inputs;
    match &mut inputs[index].data {
        RecordData::AthleteReport(data) => data.attachment_ids.get_or_insert_with(Vec::new).extend(ids),
        RecordData::NutritionEntry(data) => data.attachment_ids.get_or_insert_with(Vec::new).extend(ids),
        _ => unreachable!("linkable records are reports or nutrition entries"),
    }
    let count = attachments.len();
    let mut all = attachments;
    all.extend(inputs);
    Ok(LogPlan {
        interpretation: format!("{} with {count} attachment{}", plan.interpretation, if count == 1 { "" } else { "s" }),
        inputs: all,
        expected_current_revisions: plan.expected_current_revisions,
    })
}

// ---- Plain text -----------------------------------------------------------

const QUANTITY_ALIASES: &[(&str, PointMeasurementType)] = &[
    ("hrv rmssd", PointMeasurementType::HrvRmssd),
    ("rmssd", PointMeasurementType::HrvRmssd),
    ("sdnn", PointMeasurementType::HrvSdnn),
    ("resting heart rate", PointMeasurementType::RestingHeartRate),
    ("rhr", PointMeasurementType::RestingHeartRate),
    ("weight", PointMeasurementType::BodyWeight),
    ("body weight", PointMeasurementType::BodyWeight),
    ("respiratory rate", PointMeasurementType::RespiratoryRate),
    ("oxygen saturation", PointMeasurementType::OxygenSaturation),
    ("body temperature", PointMeasurementType::BodyTemperature),
    ("skin temperature", PointMeasurementType::SkinTemperature),
    ("body fat", PointMeasurementType::BodyFatPercentage),
];

fn input_timestamp(timestamp: &str) -> InputTimestampReceipt {
    InputTimestampReceipt {
        timestamp_text: Some(timestamp.to_string()),
        utc_offset_text: Some("Z".into()),
        utc_offset_meaning: Some(UtcOffsetMeaning::DocumentedRecordingOffset),
        source_time_zone: None,
        source_time_zone_meaning: None,
        timestamp_basis: Some(InputTimestampBasis::SourceInstant),
        timestamp_missing_reason: None,
        utc_offset_missing_reason: None,
    }
}

fn entered_by_hand() -> FieldBasis {
    FieldBasis { activity_type: None, value: Some(ValueBasis::EnteredByHand), route_summaries: None }
}

fn manual_source_id(connection: &Connection) -> Result<String> {
    connection
        .query_row(
            "SELECT sources.record_id
             FROM sources
             JOIN records ON records.record_id = sources.record_id
             WHERE sources.source_type = 'manual' AND records.is_deleted = 0
             ORDER BY records.created_at, records.record_id
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| Error::not_found("manual source is missing; run ath check"))
}

fn benchmark_by_key(connection: &Connection, key: &str) -> Result<Option<(String, String)>> {
    Ok(connection
        .query_row(
            "SELECT benchmark_definitions.record_id, benchmark_definitions.score_type
             FROM benchmark_definitions
             JOIN records ON records.record_id = benchmark_definitions.record_id
             WHERE benchmark_definitions.benchmark_key = ?1 AND records.is_deleted = 0",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

/// A benchmark key from a name: lower case, runs of other characters become
/// one hyphen, and no hyphen at either end.
pub fn normalized_benchmark_key(value: &str) -> Result<String> {
    let lower = js_trim(value).to_lowercase();
    let mut key = String::new();
    let mut in_run = false;
    for character in lower.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' || character == '-' {
            key.push(character);
            in_run = false;
        } else if !in_run {
            key.push('-');
            in_run = true;
        }
    }
    let key = key.trim_matches('-').to_string();
    if key.is_empty() {
        return Err(Error::other("benchmark name does not contain a usable name"));
    }
    Ok(key)
}

fn plan_plain_text(
    connection: &Connection,
    trimmed: &str,
    actor: &RecordActor,
    created_at: &str,
    benchmark_override: Option<&str>,
    new_id: &mut dyn FnMut() -> String,
) -> Result<LogPlan> {
    if trimmed.is_empty() {
        return Err(Error::other("nothing to log"));
    }
    if has_word(trimmed, "hrv") && !has_word(trimmed, "rmssd") && !has_word(trimmed, "sdnn") {
        return Err(Error::refused(
            "HRV is ambiguous. Say RMSSD or SDNN so the two statistics never share a baseline.",
        ));
    }
    let plan = |interpretation: String, inputs: Vec<LogRecord>| LogPlan {
        inputs,
        expected_current_revisions: Vec::new(),
        interpretation,
    };
    let record = |record_id: String, data: RecordData| LogRecord {
        record_id,
        created_at: created_at.to_string(),
        changed_by: actor.clone(),
        data,
        extensions: None,
    };

    if let Some(found) = match_measurement(trimmed) {
        let name = js_trim(found.name).to_lowercase();
        if let Some((_, quantity)) = QUANTITY_ALIASES.iter().find(|(alias, _)| *alias == name) {
            let value: f64 = found.value.parse().unwrap_or(f64::NAN);
            let expected_unit = quantity.unit();
            let input_unit = found.unit.unwrap_or(expected_unit);
            if input_unit.to_lowercase() != expected_unit.to_lowercase() {
                return Err(Error::refused(format!(
                    "{quantity} must be logged in {expected_unit}; '{input_unit}' is not converted by plain-text log"
                )));
            }
            let source_id = manual_source_id(connection)?;
            let data = PointMeasurementData {
                source_id,
                provider_record_id: None,
                provider_reporting_periods: None,
                received_at: created_at.into(),
                received_at_basis: ReceivedAtBasis::LocalWriteTime,
                supplied_fields: None,
                field_basis: Some(entered_by_hand()),
                quantity: *quantity,
                recorded_at: Some(created_at.into()),
                input_timestamp: input_timestamp(created_at),
                value: SourceValue {
                    standard_value: value,
                    standard_unit: expected_unit.into(),
                    input_value: Some(value),
                    input_unit: Some(input_unit.into()),
                    input_value_missing_reason: None,
                    input_unit_missing_reason: None,
                },
                calculations: None,
            };
            return Ok(plan(
                format!("manual {quantity} measurement"),
                vec![record(new_id(), RecordData::PointMeasurement(data))],
            ));
        }
    }

    if let Some(found) = match_result(trimmed) {
        let (minutes, seconds) = found.duration.split_once(':').unwrap_or_default();
        let duration = minutes.parse::<f64>().unwrap_or(f64::NAN) * 60.0 + seconds.parse::<f64>().unwrap_or(f64::NAN);
        let supplied_name = benchmark_override.map(str::to_string).unwrap_or_else(|| js_trim(found.name).to_string());
        let key_source = if supplied_name.is_empty() { created_at[..10].to_string() } else { supplied_name.clone() };
        let benchmark_key = normalized_benchmark_key(&key_source)?;
        let mut inputs = Vec::new();
        let benchmark_id = match benchmark_by_key(connection, &benchmark_key)? {
            Some((record_id, _)) => record_id,
            None => {
                let benchmark_id = new_id();
                inputs.push(record(
                    benchmark_id.clone(),
                    RecordData::BenchmarkDefinition(BenchmarkDefinitionData {
                        authorship: AuthoredRecordMetadata {
                            author: AuthoredRecordAuthor::Athlete,
                            authored_at: created_at.into(),
                            evidence: Vec::new(),
                            content_version: 1,
                            status: AuthoredRecordStatus::Active,
                        },
                        benchmark_key: benchmark_key.clone(),
                        benchmark_type: BenchmarkType::Custom,
                        score_type: ScoreType::Time,
                        definition: if supplied_name.is_empty() { trimmed.to_string() } else { supplied_name.clone() },
                        tags: None,
                    }),
                ));
                benchmark_id
            }
        };
        let source_id = manual_source_id(connection)?;
        let data = BenchmarkResultData {
            source_id,
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: created_at.into(),
            received_at_basis: ReceivedAtBasis::LocalWriteTime,
            supplied_fields: None,
            field_basis: Some(entered_by_hand()),
            benchmark_definition_id: benchmark_id,
            recorded_at: created_at.into(),
            input_timestamp: input_timestamp(created_at),
            score: Score { duration_s: Some(duration), reps: None, weight_kg: None },
            input_score_text: Some(found.duration.to_string()),
            scaling: found
                .scaling
                .map(|text| if text.eq_ignore_ascii_case("rx") { Scaling::Rx } else { Scaling::Scaled }),
            workout_session_id: None,
            segments: None,
        };
        inputs.push(record(new_id(), RecordData::BenchmarkResult(data)));
        return Ok(plan(format!("{benchmark_key} benchmark result"), inputs));
    }

    let report = |report_type: AthleteReportType, rating: Option<(f64, String)>| AthleteReportData {
        report_type,
        reported_at: created_at.into(),
        input_reported_at: input_timestamp(created_at),
        reported_via: ReportedVia::Text,
        rating: rating.as_ref().map(|(value, _)| *value),
        rating_scale: rating.map(|(_, scale)| scale),
        body_region: None,
        note: Some(trimmed.to_string()),
        duration_seconds: None,
        attachment_ids: None,
    };
    if let Some(found) = match_rating(trimmed) {
        let report_type = rating_report_type(js_trim(found.words));
        let rating = found.rating.parse::<f64>().unwrap_or(f64::NAN);
        let data = report(report_type, Some((rating, format!("1-{}", found.scale))));
        return Ok(plan(
            format!("{} athlete report", report_type.as_str()),
            vec![record(new_id(), RecordData::AthleteReport(data))],
        ));
    }
    let data = report(AthleteReportType::Note, None);
    Ok(plan("athlete note".into(), vec![record(new_id(), RecordData::AthleteReport(data))]))
}

fn rating_report_type(words: &str) -> AthleteReportType {
    let lower = words.to_lowercase();
    let starts = |word: &str| starts_word(&lower, word);
    let ends = |word: &str| ends_word(&lower, word);
    if starts("sore") || lower.contains("soreness") || lower.contains("aching") || ends("ache") {
        AthleteReportType::Soreness
    } else if starts("stress") || ends("stressed") {
        AthleteReportType::Stress
    } else if has_word(&lower, "mood") {
        AthleteReportType::Mood
    } else if starts("energy") || ends("energetic") {
        AthleteReportType::Energy
    } else {
        AthleteReportType::Note
    }
}

// ---- Structured batches -----------------------------------------------------

const STRUCTURED_LOG_FIELDS: &[&str] =
    &["record_type", "record_id", "created_at", "current_revision", "is_deleted", "data", "extensions"];

fn structured_issue(index: usize, message: impl std::fmt::Display) -> Error {
    Error::refused(format!("structured log item {} {message}", index + 1))
}

/// A value typed through `ath log` was entered by hand, not read from a device
/// file (D187). An explicit basis is kept.
fn with_entered_basis(data: RecordData) -> RecordData {
    fn mark(basis: &mut Option<FieldBasis>) {
        let basis = basis.get_or_insert(FieldBasis { activity_type: None, value: None, route_summaries: None });
        if basis.value.is_none() {
            basis.value = Some(ValueBasis::EnteredByHand);
        }
    }
    let mut data = data;
    match &mut data {
        RecordData::PointMeasurement(record) => mark(&mut record.field_basis),
        RecordData::MeasurementSample(record) => mark(&mut record.field_basis),
        RecordData::SleepSession(record) => mark(&mut record.field_basis),
        RecordData::WorkoutSession(record) => mark(&mut record.field_basis),
        RecordData::BenchmarkResult(record) => mark(&mut record.field_basis),
        RecordData::ProviderSummary(record) => mark(&mut record.field_basis),
        RecordData::ProviderScore(record) => mark(&mut record.field_basis),
        _ => {}
    }
    data
}

fn issue_text(error: &ValidationError) -> String {
    error
        .issues
        .iter()
        .map(|issue| {
            let path = issue.path.iter().map(ToString::to_string).collect::<Vec<_>>().join(".");
            if path.is_empty() {
                issue.message.clone()
            } else {
                format!("{path}: {}", issue.message)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn plan_structured(
    value: &Value,
    actor: &RecordActor,
    created_at: &str,
    new_id: &mut dyn FnMut() -> String,
) -> Result<LogPlan> {
    let items: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    if items.is_empty() {
        return Err(Error::refused("structured log array cannot be empty"));
    }
    let mut inputs = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Value::Object(raw) = item else {
            return Err(structured_issue(index, "must be an object"));
        };
        let unknown: Vec<&str> = json::ordered_keys(raw, false)
            .into_iter()
            .map(String::as_str)
            .filter(|key| !STRUCTURED_LOG_FIELDS.contains(key))
            .collect();
        if !unknown.is_empty() {
            return Err(structured_issue(
                index,
                format!("has unknown field{}: {}", if unknown.len() == 1 { "" } else { "s" }, unknown.join(", ")),
            ));
        }
        let type_value = raw.get("record_type").cloned().unwrap_or(Value::Null);
        RecordType::schema().validate(&type_value)?;
        let record_type = type_value.as_str().and_then(RecordType::parse).expect("validated record type");
        if matches!(record_type, RecordType::PredictionResultLink | RecordType::PredictionGrade) {
            return Err(Error::refused(format!(
                "structured log cannot create {record_type}; use ath grade so the tool validates and writes it"
            )));
        }
        if raw.get("current_revision").is_some_and(|revision| revision.as_f64() != Some(1.0)) {
            return Err(structured_issue(index, "can only create revision 1"));
        }
        if raw.get("is_deleted").is_some_and(|deleted| deleted != &Value::Bool(false)) {
            return Err(structured_issue(index, "cannot create a deleted record"));
        }
        let record_id = match raw.get("record_id") {
            Some(Value::String(id)) => id.clone(),
            _ => new_id(),
        };
        let item_created_at = match raw.get("created_at") {
            Some(Value::String(text)) => parse_utc_timestamp(text)?,
            _ => created_at.to_string(),
        };
        let mut candidate = Map::new();
        candidate.insert("record_id".into(), Value::String(record_id));
        candidate.insert("record_type".into(), Value::String(record_type.as_str().into()));
        candidate.insert("created_at".into(), Value::String(item_created_at));
        candidate.insert("current_revision".into(), Value::from(1));
        candidate.insert("is_deleted".into(), Value::Bool(false));
        candidate.insert("data".into(), raw.get("data").cloned().unwrap_or(Value::Null));
        if let Some(extensions) = raw.get("extensions") {
            candidate.insert("extensions".into(), extensions.clone());
        }
        let parsed = CurrentRecord::from_value(&Value::Object(candidate))
            .map_err(|error| structured_issue(index, format!("is invalid: {}", issue_text(&error))))?;
        inputs.push(LogRecord {
            record_id: parsed.record_id,
            created_at: parsed.created_at,
            changed_by: actor.clone(),
            data: with_entered_basis(parsed.data),
            extensions: parsed.extensions,
        });
    }
    let count = inputs.len();
    Ok(LogPlan {
        interpretation: format!("{count} structured record{}", if count == 1 { "" } else { "s" }),
        inputs,
        expected_current_revisions: Vec::new(),
    })
}

// ---- Automatic grading --------------------------------------------------------

/// Link and grade a new result when its benchmark has exactly one open
/// prediction: active, current, and not yet linked at its current revision.
fn add_automatic_grades(
    connection: &Connection,
    plan: LogPlan,
    actor: &RecordActor,
    created_at: &str,
    new_id: &mut dyn FnMut() -> String,
) -> Result<LogPlan> {
    let mut additions = Vec::new();
    let mut expected = plan.expected_current_revisions.clone();
    for input in &plan.inputs {
        let RecordData::BenchmarkResult(result) = &input.data else {
            continue;
        };
        let mut statement = connection.prepare_cached(
            "SELECT predictions.record_id
             FROM predictions
             JOIN records ON records.record_id = predictions.record_id
             WHERE records.is_deleted = 0
               AND predictions.benchmark_definition_id = ?1
               AND predictions.status = 'active'
               AND NOT EXISTS (
                 SELECT 1
                 FROM prediction_result_links
                 JOIN records AS link_records ON link_records.record_id = prediction_result_links.record_id
                 WHERE link_records.is_deleted = 0
                   AND prediction_result_links.prediction_id = predictions.record_id
                   AND prediction_result_links.prediction_revision = records.current_revision
               )
             ORDER BY predictions.authored_at DESC, predictions.record_id
             LIMIT 2",
        )?;
        let open: Vec<String> = statement
            .query_map([&result.benchmark_definition_id], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let [prediction_id] = &open[..] else {
            continue;
        };
        let Some(prediction_record) = crate::store::read::get_record(connection, prediction_id)? else {
            continue;
        };
        let RecordData::Prediction(prediction) = &prediction_record.data else {
            continue;
        };
        if prediction_record.is_deleted {
            continue;
        }
        let score_type: Option<String> = connection
            .query_row(
                "SELECT score_type FROM benchmark_definitions WHERE record_id = ?1",
                [&result.benchmark_definition_id],
                |row| row.get(0),
            )
            .optional()?;
        let score_type =
            score_type.ok_or_else(|| Error::other("benchmark definition is missing while planning automatic grade"))?;
        let arithmetic = grade_arithmetic(prediction, &result.score, &score_type).map_err(|message| {
            let mut characters = message.chars();
            Error::other(match characters.next() {
                Some(first) => first.to_lowercase().chain(characters).collect(),
                None => message,
            })
        })?;
        let link_id = new_id();
        let link_reference = EvidenceReference { record_id: link_id.clone(), revision: 1 };
        additions.push(LogRecord {
            record_id: link_id,
            created_at: created_at.into(),
            changed_by: actor.clone(),
            data: RecordData::PredictionResultLink(PredictionResultLinkData {
                prediction: EvidenceReference {
                    record_id: prediction_record.record_id.clone(),
                    revision: prediction_record.current_revision,
                },
                benchmark_result: EvidenceReference { record_id: input.record_id.clone(), revision: 1 },
                created_by: actor.clone(),
                linked_at: created_at.into(),
                content_version: 1,
                status: AuthoredRecordStatus::Active,
            }),
            extensions: None,
        });
        additions.push(LogRecord {
            record_id: new_id(),
            created_at: created_at.into(),
            changed_by: crate::prediction::grade::tool_actor(),
            data: RecordData::PredictionGrade(PredictionGradeData {
                authorship: AuthoredRecordMetadata {
                    author: AuthoredRecordAuthor::Tool {
                        tool: "ath".into(),
                        tool_version: ATHLETIC_STANDARD_VERSION.into(),
                        calculation_version: DEFAULT_GRADE_CALCULATION_VERSION.into(),
                    },
                    authored_at: created_at.into(),
                    evidence: vec![link_reference.clone()],
                    content_version: 1,
                    status: AuthoredRecordStatus::Active,
                },
                prediction_result_link: link_reference,
                signed_error: arithmetic.signed_error,
                absolute_error_percentage: arithmetic.absolute_error_percentage,
                result_in_predicted_range: arithmetic.result_in_predicted_range,
                predicted_score: prediction.predicted_score.clone(),
                actual_score: result.score.clone(),
            }),
            extensions: None,
        });
        expected.push((prediction_record.record_id.clone(), prediction_record.current_revision));
    }
    if additions.is_empty() {
        return Ok(plan);
    }
    let count = additions.len() / 2;
    let mut inputs = plan.inputs;
    inputs.extend(additions);
    Ok(LogPlan {
        interpretation: format!("{}; added {count} unambiguous prediction link and grade", plan.interpretation),
        inputs,
        expected_current_revisions: expected,
    })
}

// ---- Text matching ------------------------------------------------------------
//
// The 2.1.0 program read plain text with three regular expressions. These
// functions follow the same leftmost, lazy-then-greedy choices, so the same
// text gives the same parts.

/// JavaScript `\s`.
fn js_space(character: char) -> bool {
    matches!(
        character,
        '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// A JavaScript line terminator, which `.` does not match.
fn line_terminator(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `String.prototype.trim`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(js_space)
}

fn word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn boundary_before(text: &str, at: usize) -> bool {
    !text[..at].chars().next_back().is_some_and(word_character)
}

fn boundary_after(text: &str, at: usize) -> bool {
    !text[at..].chars().next().is_some_and(word_character)
}

/// `/\bword\b/i`.
fn has_word(text: &str, word: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.match_indices(word).any(|(at, _)| boundary_before(&lower, at) && boundary_after(&lower, at + word.len()))
}

/// `/\bword/i`.
fn starts_word(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(at, _)| boundary_before(lower, at))
}

/// `/word\b/i`.
fn ends_word(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(at, _)| boundary_after(lower, at + word.len()))
}

fn run_end(text: &str, start: usize, test: impl Fn(char) -> bool) -> usize {
    text[start..]
        .char_indices()
        .find(|(_, character)| !test(*character))
        .map_or(text.len(), |(offset, _)| start + offset)
}

/// End positions of `\s+` from `start`, longest first.
fn space_runs(text: &str, start: usize) -> Vec<usize> {
    let end = run_end(text, start, js_space);
    let mut positions: Vec<usize> =
        text[start..end].char_indices().map(|(offset, character)| start + offset + character.len_utf8()).collect();
    positions.reverse();
    positions
}

/// End positions of `\d+` from `start`, longest first.
fn digit_runs(text: &str, start: usize) -> Vec<usize> {
    let end = run_end(text, start, |character| character.is_ascii_digit());
    (start + 1..=end).rev().collect()
}

/// End positions of `\d+(?:\.\d+)?` from `start`, in the order the regex tries them.
fn number_ends(text: &str, start: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    for digits in digit_runs(text, start) {
        if text[digits..].starts_with('.') {
            ends.extend(digit_runs(text, digits + 1));
        }
        ends.push(digits);
    }
    ends
}

/// Lazy group starts: every boundary from `first`, stopping at a line terminator.
fn lazy_ends(text: &str, first: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut position = first;
    loop {
        if position > text.len() {
            break;
        }
        ends.push(position);
        match text[position..].chars().next() {
            Some(character) if !line_terminator(character) => position += character.len_utf8(),
            _ => break,
        }
    }
    ends
}

/// True when `(?:\s+.*)?$` matches from `start`.
fn optional_rest_of_line(text: &str, start: usize) -> bool {
    if start == text.len() {
        return true;
    }
    space_runs(text, start).into_iter().any(|after| !text[after..].chars().any(line_terminator))
}

struct MeasurementMatch<'a> {
    name: &'a str,
    value: &'a str,
    unit: Option<&'a str>,
}

/// `/^(.+?)\s+(-?\d+(?:\.\d+)?)\s*(\S+)?(?:\s+.*)?$/i`
fn match_measurement(text: &str) -> Option<MeasurementMatch<'_>> {
    let first = text.chars().next().filter(|character| !line_terminator(*character))?.len_utf8();
    for name_end in lazy_ends(text, first) {
        for space_end in space_runs(text, name_end) {
            let number_start = if text[space_end..].starts_with('-') { space_end + 1 } else { space_end };
            for number_end in number_ends(text, number_start) {
                for gap_end in std::iter::once(run_end(text, number_end, js_space))
                    .chain(space_runs(text, number_end).into_iter().skip(1))
                    .chain(std::iter::once(number_end))
                {
                    let unit_end = run_end(text, gap_end, |character| !js_space(character));
                    let mut units: Vec<Option<usize>> = text[gap_end..unit_end]
                        .char_indices()
                        .map(|(offset, c)| Some(gap_end + offset + c.len_utf8()))
                        .collect();
                    units.reverse();
                    units.push(None);
                    for unit in units {
                        let after = unit.unwrap_or(gap_end);
                        if optional_rest_of_line(text, after) {
                            return Some(MeasurementMatch {
                                name: &text[..name_end],
                                value: &text[space_end..number_end],
                                unit: unit.map(|end| &text[gap_end..end]),
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

struct ResultMatch<'a> {
    name: &'a str,
    duration: &'a str,
    scaling: Option<&'a str>,
}

fn starts_with_ignore_case(text: &str, at: usize, word: &str) -> bool {
    text.get(at..at + word.len()).is_some_and(|part| part.eq_ignore_ascii_case(word))
}

/// `\s+(\d+:[0-5]\d)(?:\s+(rx|scaled))?$` from `start`.
fn match_duration_tail(text: &str, start: usize) -> Option<(usize, usize, Option<&str>)> {
    for space_end in space_runs(text, start) {
        for digits_end in digit_runs(text, space_end) {
            let bytes = text.as_bytes();
            if bytes.get(digits_end) != Some(&b':')
                || !bytes.get(digits_end + 1).is_some_and(|byte| (b'0'..=b'5').contains(byte))
                || !bytes.get(digits_end + 2).is_some_and(u8::is_ascii_digit)
            {
                continue;
            }
            let duration_end = digits_end + 3;
            for scaling_space in space_runs(text, duration_end) {
                for word in ["rx", "scaled"] {
                    if starts_with_ignore_case(text, scaling_space, word) && scaling_space + word.len() == text.len() {
                        return Some((space_end, duration_end, Some(&text[scaling_space..])));
                    }
                }
            }
            if duration_end == text.len() {
                return Some((space_end, duration_end, None));
            }
        }
    }
    None
}

/// `/^(.*?)(?:\s+in)?\s+(\d+:[0-5]\d)(?:\s+(rx|scaled))?$/i`
fn match_result(text: &str) -> Option<ResultMatch<'_>> {
    for name_end in lazy_ends(text, 0) {
        let mut starts: Vec<usize> = space_runs(text, name_end)
            .into_iter()
            .filter(|after| starts_with_ignore_case(text, *after, "in"))
            .map(|after| after + 2)
            .collect();
        starts.push(name_end);
        for start in starts {
            if let Some((duration_start, duration_end, scaling)) = match_duration_tail(text, start) {
                return Some(ResultMatch {
                    name: &text[..name_end],
                    duration: &text[duration_start..duration_end],
                    scaling,
                });
            }
        }
    }
    None
}

struct RatingMatch<'a> {
    words: &'a str,
    rating: &'a str,
    scale: &'a str,
}

/// `/^(.*?)(\d+(?:\.\d+)?)\s*\/\s*(\d+(?:\.\d+)?)$/i`
fn match_rating(text: &str) -> Option<RatingMatch<'_>> {
    for words_end in lazy_ends(text, 0) {
        for rating_end in number_ends(text, words_end) {
            let slash = run_end(text, rating_end, js_space);
            if !text[slash..].starts_with('/') {
                continue;
            }
            let scale_start = run_end(text, slash + 1, js_space);
            if number_ends(text, scale_start).contains(&text.len()) {
                return Some(RatingMatch {
                    words: &text[..words_end],
                    rating: &text[words_end..rating_end],
                    scale: &text[scale_start..],
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(text: &str) -> Option<(String, String, Option<String>)> {
        match_measurement(text).map(|found| (found.name.into(), found.value.into(), found.unit.map(str::to_string)))
    }

    #[test]
    fn measurements_split_like_the_old_expression() {
        let parts =
            |name: &str, value: &str, unit: Option<&str>| Some((name.into(), value.into(), unit.map(str::to_string)));
        assert_eq!(measurement("RMSSD 61 ms"), parts("RMSSD", "61", Some("ms")));
        assert_eq!(measurement("hrv rmssd 61"), parts("hrv rmssd", "61", None));
        assert_eq!(measurement("weight 70.5kg"), parts("weight", "70.5", Some("kg")));
        assert_eq!(measurement("weight -5 kg after run"), parts("weight", "-5", Some("kg")));
        assert_eq!(measurement("body fat 15 %"), parts("body fat", "15", Some("%")));
        assert_eq!(measurement("felt good"), None);
        assert_eq!(measurement("sore 2 days 6/10"), parts("sore", "2", Some("days")));
    }

    #[test]
    fn results_and_ratings_split_like_the_old_expressions() {
        let result = |text: &str| {
            match_result(text)
                .map(|found| (found.name.to_string(), found.duration.to_string(), found.scaling.map(str::to_string)))
        };
        assert_eq!(result("Fran in 5:00 rx"), Some(("Fran".into(), "5:00".into(), Some("rx".into()))));
        assert_eq!(result("fran 3:15 RX"), Some(("fran".into(), "3:15".into(), Some("RX".into()))));
        assert_eq!(result("long run 75:30"), Some(("long run".into(), "75:30".into(), None)));
        assert_eq!(result("3:15"), None);
        assert_eq!(result("fran 3:75"), None);
        let rating = |text: &str| {
            match_rating(text).map(|found| (found.words.to_string(), found.rating.to_string(), found.scale.to_string()))
        };
        assert_eq!(rating("sore legs 6/10"), Some(("sore legs ".into(), "6".into(), "10".into())));
        assert_eq!(rating("energy 7.5 / 10"), Some(("energy ".into(), "7.5".into(), "10".into())));
        assert_eq!(rating("a12/10"), Some(("a".into(), "12".into(), "10".into())));
        assert_eq!(rating("6/10 later"), None);
        assert_eq!(rating_report_type("legs ache"), AthleteReportType::Soreness);
        assert_eq!(rating_report_type("unstressed"), AthleteReportType::Stress);
        assert_eq!(rating_report_type("moody"), AthleteReportType::Note);
    }

    #[test]
    fn benchmark_keys_are_normalized() {
        assert_eq!(normalized_benchmark_key(" My Workout! ").unwrap(), "my-workout");
        assert_eq!(normalized_benchmark_key("5k_run").unwrap(), "5k_run");
        assert!(normalized_benchmark_key("!!!").is_err());
        assert!(has_word("HRV 61", "hrv"));
        assert!(!has_word("hrvx 61", "hrv"));
    }
}
