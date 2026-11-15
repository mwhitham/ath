//! Offline prediction evidence: what the database knew at a cut-off.
//!
//! Evidence uses each record's revision known at the cut-off and only events
//! at or before it. Device measurements from manual sources are left out.
//! Athlete reports appear as reports; they can explain a miss or change
//! confidence, but they never become measured evidence. Building evidence
//! needs no key and no network.

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::json::Value;
use crate::model::{
    js_date_parse, CurrentRecord, EvidenceReference, RecordData, RecordErasureMarker, RecordRevision, RecordType,
    Shape, SourceType,
};
use crate::store::query::{parse_utc_timestamp, query_records_as_of, AsOfQuery};
use crate::store::read;
use crate::store::Store;

pub const DEFAULT_EVIDENCE_LIMIT: u32 = 100_000;
pub const DEFAULT_WINDOW_DAYS: u32 = 90;

/// Which benchmark, as of when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRequest {
    /// A benchmark definition record ID or benchmark key.
    pub benchmark: String,
    /// The cut-off in the stored UTC form.
    pub as_of: String,
    /// The as-of query limit. 100,000 by default.
    pub limit: Option<u32>,
    /// Days of measurements, reports, and sessions before the cut-off. 90 by default.
    pub window_days: Option<u32>,
}

impl EvidenceRequest {
    pub fn new(benchmark: impl Into<String>, as_of: impl Into<String>) -> Self {
        EvidenceRequest { benchmark: benchmark.into(), as_of: as_of.into(), limit: None, window_days: None }
    }
}

/// The `ath predict --json` evidence object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionEvidence {
    /// Always `prediction_evidence`.
    pub purpose: &'static str,
    /// Always false: this is evidence, not a prediction.
    pub is_prediction: bool,
    pub as_of: String,
    #[serde(serialize_with = "crate::view::serialize_record")]
    pub benchmark: CurrentRecord,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub prior_results: Vec<CurrentRecord>,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub device_measurements: Vec<CurrentRecord>,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub athlete_reports: Vec<CurrentRecord>,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub sleep_sessions: Vec<CurrentRecord>,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub workout_sessions: Vec<CurrentRecord>,
    #[serde(serialize_with = "crate::view::serialize_records")]
    pub prior_predictions: Vec<CurrentRecord>,
    pub gaps: Vec<String>,
    /// Present in `ath predict --json` when `--as-of` named a calendar day.
    /// Never part of the text sent to a model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_time_zone: Option<String>,
}

impl PredictionEvidence {
    /// The exact record revisions a prediction from this evidence rests on:
    /// the benchmark, prior results, measurements, reports, and sessions.
    pub fn references(&self) -> Vec<EvidenceReference> {
        std::iter::once(&self.benchmark)
            .chain(&self.prior_results)
            .chain(&self.device_measurements)
            .chain(&self.athlete_reports)
            .chain(&self.sleep_sessions)
            .chain(&self.workout_sessions)
            .map(|record| EvidenceReference { record_id: record.record_id.clone(), revision: record.current_revision })
            .collect()
    }

    /// The compact JSON a model receives. It never includes the calendar timezone.
    pub fn model_text(&self) -> String {
        let mut evidence = self.clone();
        evidence.calendar_time_zone = None;
        crate::json::encode(&evidence)
    }
}

/// The event time of a record, when its type has one.
pub fn event_at(record: &CurrentRecord) -> Option<&str> {
    match &record.data {
        RecordData::PointMeasurement(data) => data.recorded_at.as_deref(),
        RecordData::AthleteReport(data) => Some(&data.reported_at),
        RecordData::SleepSession(data) => Some(&data.start_at),
        RecordData::WorkoutSession(data) => Some(&data.start_at),
        RecordData::BenchmarkResult(data) => Some(&data.recorded_at),
        RecordData::Prediction(data) => Some(&data.authorship.authored_at),
        _ => None,
    }
}

fn benchmark_key(record: &CurrentRecord) -> Option<&str> {
    match &record.data {
        RecordData::BenchmarkDefinition(data) => Some(&data.benchmark_key),
        _ => None,
    }
}

fn benchmark_id(record: &CurrentRecord) -> Option<&str> {
    match &record.data {
        RecordData::BenchmarkResult(data) => Some(&data.benchmark_definition_id),
        RecordData::Prediction(data) => Some(&data.benchmark_definition_id),
        _ => None,
    }
}

const EVIDENCE_TYPES: &[RecordType] = &[
    RecordType::BenchmarkDefinition,
    RecordType::Source,
    RecordType::PointMeasurement,
    RecordType::AthleteReport,
    RecordType::SleepSession,
    RecordType::WorkoutSession,
    RecordType::BenchmarkResult,
    RecordType::Prediction,
];

/// Build evidence for one benchmark at a cut-off.
pub fn build_evidence(store: &Store, request: &EvidenceRequest) -> Result<PredictionEvidence> {
    let as_of = parse_utc_timestamp(&request.as_of)?;
    let limit = request.limit.unwrap_or(DEFAULT_EVIDENCE_LIMIT);
    let window_days = request.window_days.unwrap_or(DEFAULT_WINDOW_DAYS);
    if !(1..=3_650).contains(&window_days) {
        return Err(Error::other("Evidence window must be between 1 and 3650 days"));
    }
    let records = query_records_as_of(
        store.connection(),
        &AsOfQuery { as_of: as_of.clone(), record_types: EVIDENCE_TYPES.to_vec(), limit: Some(limit) },
    )?;
    let Some(benchmark) = records
        .iter()
        .find(|record| {
            benchmark_key(record).is_some_and(|key| record.record_id == request.benchmark || key == request.benchmark)
        })
        .cloned()
    else {
        let mut available: Vec<&str> = records.iter().filter_map(benchmark_key).collect();
        available.sort_unstable();
        let available = if available.is_empty() { "none".to_string() } else { available.join(", ") };
        return Err(Error::not_found(format!(
            "no benchmark '{}' was visible at {as_of}. Available: {available}",
            request.benchmark
        )));
    };

    let manual_sources: std::collections::HashSet<&str> = records
        .iter()
        .filter(|record| matches!(&record.data, RecordData::Source(data) if data.source_type == SourceType::Manual))
        .map(|record| record.record_id.as_str())
        .collect();
    let as_of_millis = js_date_parse(&as_of).unwrap_or_default();
    let window_start = as_of_millis - i64::from(window_days) * 86_400_000;
    let visible_by_event = |record: &CurrentRecord| event_at(record).is_none_or(|at| at <= as_of.as_str());
    let within_window = |record: &CurrentRecord| {
        event_at(record)
            .is_some_and(|at| at <= as_of.as_str() && js_date_parse(at).is_some_and(|millis| millis >= window_start))
    };
    let of_type = |record_type: RecordType| records.iter().filter(move |record| record.record_type() == record_type);
    let mut device_measurements: Vec<CurrentRecord> = of_type(RecordType::PointMeasurement)
        .filter(|record| visible_by_event(record) && within_window(record))
        .filter(|record| !record.data.source_id().is_some_and(|source| manual_sources.contains(source)))
        .cloned()
        .collect();
    let windowed = |record_type: RecordType| -> Vec<CurrentRecord> {
        of_type(record_type).filter(|record| within_window(record)).cloned().collect()
    };
    let mut athlete_reports = windowed(RecordType::AthleteReport);
    let mut sleep_sessions = windowed(RecordType::SleepSession);
    let mut workout_sessions = windowed(RecordType::WorkoutSession);
    let for_benchmark = |record_type: RecordType| -> Vec<CurrentRecord> {
        of_type(record_type)
            .filter(|record| benchmark_id(record) == Some(benchmark.record_id.as_str()) && visible_by_event(record))
            .cloned()
            .collect()
    };
    let mut prior_results = for_benchmark(RecordType::BenchmarkResult);
    let mut prior_predictions = for_benchmark(RecordType::Prediction);

    let mut gaps = Vec::new();
    if prior_results.is_empty() {
        gaps.push("No earlier result exists for this benchmark.".to_string());
    }
    if device_measurements.is_empty() {
        gaps.push(format!("No device point measurements exist in the prior {window_days} days."));
    }
    if sleep_sessions.is_empty() {
        gaps.push(format!("No recorded sleep session exists in the prior {window_days} days."));
    }
    if records.len() == limit as usize {
        gaps.push(format!("Evidence reached the {limit}-record query limit."));
    }
    for list in [
        &mut prior_results,
        &mut device_measurements,
        &mut athlete_reports,
        &mut sleep_sessions,
        &mut workout_sessions,
        &mut prior_predictions,
    ] {
        list.sort_by(|left, right| {
            event_at(left)
                .unwrap_or("")
                .cmp(event_at(right).unwrap_or(""))
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
    }
    Ok(PredictionEvidence {
        purpose: "prediction_evidence",
        is_prediction: false,
        as_of,
        benchmark,
        prior_results,
        device_measurements,
        athlete_reports,
        sleep_sessions,
        workout_sessions,
        prior_predictions,
        gaps,
        calendar_time_zone: None,
    })
}

/// What an evidence reference resolves to now. It is returned one at a
/// time, so the size of the available variant does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EvidenceReferenceResolution {
    /// The exact revision is retained.
    Available {
        reference: EvidenceReference,
        #[serde(serialize_with = "serialize_revision")]
        revision: RecordRevision,
    },
    /// The record was erased; only its value-free marker remains.
    Erased { reference: EvidenceReference, erasure_marker: RecordErasureMarker },
    /// The record or revision does not exist.
    Missing { reference: EvidenceReference },
}

fn serialize_revision<S: serde::Serializer>(revision: &RecordRevision, serializer: S) -> Result<S::Ok, S::Error> {
    revision.to_value().serialize(serializer)
}

/// Resolve an evidence reference: the exact revision, an erasure marker, or missing.
pub fn resolve_evidence_reference(store: &Store, reference: &EvidenceReference) -> Result<EvidenceReferenceResolution> {
    let connection = store.connection();
    EvidenceReference::schema().validate(&crate::json::to_value(reference))?;
    let erased: Option<(String, String, String)> = connection
        .query_row(
            "SELECT record_id, erased_at, reason_code FROM record_erasure_markers WHERE record_id = ?1",
            [&reference.record_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((record_id, erased_at, reason_code)) = erased {
        let mut marker = crate::json::Map::new();
        marker.insert("record_id".into(), Value::String(record_id));
        marker.insert("erased_at".into(), Value::String(erased_at));
        marker.insert("reason_code".into(), Value::String(reason_code));
        let marker = Value::Object(marker);
        RecordErasureMarker::schema().validate(&marker)?;
        let erasure_marker: RecordErasureMarker =
            serde_json::from_value(marker).map_err(|error| Error::other(error.to_string()))?;
        return Ok(EvidenceReferenceResolution::Erased { reference: reference.clone(), erasure_marker });
    }
    match read::revision(connection, &reference.record_id, reference.revision)? {
        Some(revision) => Ok(EvidenceReferenceResolution::Available { reference: reference.clone(), revision }),
        None => Ok(EvidenceReferenceResolution::Missing { reference: reference.clone() }),
    }
}
