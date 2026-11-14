//! `ath backtest`: replay history and measure prediction error.
//!
//! Each benchmark's first current result is an anchor, not a test case: there
//! is nothing earlier to predict from. Every later result is replayed. The
//! replay shows a model the evidence `ath predict --as-of` would have shown
//! one millisecond before the result: before it was recorded and before this
//! database first learned it. That cut-off hides later events, revisions
//! learned later, and every revision of the result itself.
//!
//! A backtest never writes athlete data. Its report is a separate file. A
//! model that fails on one replay is recorded in the report; the other
//! replays and models continue.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use super::client::{CompleteRequest, Gateway, ModelClient};
use super::evidence::{build_evidence, EvidenceRequest};
use super::predict::strip_code_fence;
use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::grade::js_round;
use crate::model::{time, CurrentRecord, RecordData, Score, ATHLETIC_STANDARD_VERSION};
use crate::store::read::current_from_row;

pub const BACKTEST_SYSTEM_PROMPT: &str =
    "Return only JSON with predicted_score, optional predicted_range, confidence, and reasoning.";

/// One model's attempt at one result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestReplay {
    pub model: String,
    pub benchmark_result_id: String,
    pub benchmark_key: String,
    /// The replay cut-off.
    pub as_of: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_value: Option<f64>,
    pub actual_value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute_error_percentage: Option<f64>,
    /// The model failure, when this replay failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestModelSummary {
    pub model: String,
    pub completed: usize,
    pub failed: usize,
    /// Null when no replay completed.
    pub median_absolute_error_percentage: Option<f64>,
}

/// The backtest report file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestReport {
    pub athleticstandard_version: &'static str,
    pub created_at: String,
    pub models: Vec<String>,
    pub replayable_result_count: usize,
    pub skipped_first_result_count: usize,
    pub replays: Vec<BacktestReplay>,
    pub summary: Vec<BacktestModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktestOptions {
    pub models: Vec<String>,
    pub gateway: Gateway,
    /// The report time. Now by default.
    pub created_at: Option<String>,
}

/// A result to replay and its cut-off.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayTarget {
    pub result: CurrentRecord,
    pub definition: CurrentRecord,
    pub as_of: String,
}

fn score_value(score: &Score, score_type: &str) -> Result<f64> {
    score.value_for(score_type).ok_or_else(|| Error::other(format!("score does not contain {score_type}")))
}

fn score_from_value(value: &Value, score_type: &str) -> Result<f64> {
    let field = match score_type {
        "time" => "duration_s",
        "reps" => "reps",
        _ => "weight_kg",
    };
    value
        .get(field)
        .filter(|value| !value.is_null())
        .and_then(Value::as_f64)
        .ok_or_else(|| Error::other(format!("score does not contain {score_type}")))
}

/// Read a model's predicted value from its reply.
pub fn parse_model_score(content: &str, score_type: &str) -> Result<f64> {
    let parsed = json::parse(strip_code_fence(content)).map_err(|error| Error::other(error.message))?;
    match parsed.get("predicted_score") {
        Some(score @ Value::Object(_)) => score_from_value(score, score_type),
        _ => Err(Error::other("model response has no predicted_score object")),
    }
}

/// The median, rounded to one decimal place. Null for no values.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let value = if sorted.len() % 2 == 1 { sorted[middle] } else { (sorted[middle - 1] + sorted[middle]) / 2.0 };
    Some(js_round(value * 10.0) / 10.0)
}

fn result_data(record: &CurrentRecord) -> Option<&crate::model::BenchmarkResultData> {
    match &record.data {
        RecordData::BenchmarkResult(data) => Some(data),
        _ => None,
    }
}

fn definition_data(record: &CurrentRecord) -> Option<&crate::model::BenchmarkDefinitionData> {
    match &record.data {
        RecordData::BenchmarkDefinition(data) => Some(data),
        _ => None,
    }
}

/// The replay cut-off for a result: one millisecond before the earlier of its
/// event time and the time this database first learned it.
pub fn replay_cut_off(connection: &Connection, result: &CurrentRecord) -> Result<String> {
    let first_known: Option<String> = connection
        .query_row(
            "SELECT became_known_at FROM record_revisions WHERE record_id = ?1 ORDER BY revision LIMIT 1",
            [&result.record_id],
            |row| row.get(0),
        )
        .ok();
    let first_known =
        first_known.ok_or_else(|| Error::other(format!("result revision is missing: {}", result.record_id)))?;
    let known = time::parse_utc_timestamp(&first_known)
        .ok_or_else(|| Error::other(format!("result revision is missing: {}", result.record_id)))?;
    let recorded = result_data(result).and_then(|data| time::parse_utc_timestamp(&data.recorded_at)).unwrap_or(known);
    Ok(time::format_utc_millis(known.min(recorded) - 1))
}

/// Every replayable result, and the count of first results skipped.
pub fn replay_targets(connection: &Connection) -> Result<(Vec<ReplayTarget>, usize)> {
    let mut statement = connection.prepare(
        "SELECT records.record_id, records.record_type, records.current_revision, records.created_at,
                records.is_deleted, record_revisions.record_json, record_revisions.extensions_json
         FROM records
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         WHERE records.record_type IN ('benchmark_definition', 'benchmark_result')
         ORDER BY records.created_at, records.record_id",
    )?;
    let mut rows = statement.query([])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        records.push(current_from_row(row)?);
    }
    let definitions: std::collections::HashMap<&str, &CurrentRecord> = records
        .iter()
        .filter(|record| definition_data(record).is_some())
        .map(|record| (record.record_id.as_str(), record))
        .collect();
    let mut groups: Vec<(String, Vec<&CurrentRecord>)> = Vec::new();
    for record in &records {
        let Some(data) = result_data(record) else {
            continue;
        };
        if record.is_deleted {
            continue;
        }
        match groups.iter_mut().find(|group| group.0 == data.benchmark_definition_id) {
            Some(group) => group.1.push(record),
            None => groups.push((data.benchmark_definition_id.clone(), vec![record])),
        }
    }
    let mut targets = Vec::new();
    let mut skipped_first = 0;
    for (benchmark_id, mut results) in groups {
        let definition = definitions
            .get(benchmark_id.as_str())
            .ok_or_else(|| Error::other(format!("benchmark definition is missing: {benchmark_id}")))?;
        results.sort_by(|left, right| {
            let at =
                |record: &CurrentRecord| result_data(record).map(|data| data.recorded_at.clone()).unwrap_or_default();
            at(left).cmp(&at(right)).then_with(|| left.record_id.cmp(&right.record_id))
        });
        skipped_first += results.len().min(1);
        for result in results.into_iter().skip(1) {
            targets.push(ReplayTarget {
                result: result.clone(),
                definition: (*definition).clone(),
                as_of: replay_cut_off(connection, result)?,
            });
        }
    }
    Ok((targets, skipped_first))
}

/// Replay every later result with every model. Writes nothing.
pub fn run_backtest(
    connection: &Connection,
    options: &BacktestOptions,
    client: &mut dyn ModelClient,
) -> Result<BacktestReport> {
    if options.models.is_empty() {
        return Err(Error::other("backtest needs at least one model"));
    }
    let (targets, skipped_first) = replay_targets(connection)?;
    let mut replays = Vec::new();
    for target in &targets {
        let evidence = build_evidence(connection, &EvidenceRequest::new(&target.definition.record_id, &target.as_of))?;
        let definition = definition_data(&target.definition).expect("targets name definitions");
        let score_type = definition.score_type.as_str();
        let result = result_data(&target.result).expect("targets name results");
        let actual = score_value(&result.score, score_type)?;
        let user = evidence.model_text();
        for model in &options.models {
            let outcome = client
                .complete(&CompleteRequest {
                    gateway: options.gateway,
                    model: model.clone(),
                    system: BACKTEST_SYSTEM_PROMPT.to_string(),
                    user: user.clone(),
                })
                .and_then(|response| parse_model_score(&response.content, score_type));
            let base = BacktestReplay {
                model: model.clone(),
                benchmark_result_id: target.result.record_id.clone(),
                benchmark_key: definition.benchmark_key.clone(),
                as_of: target.as_of.clone(),
                predicted_value: None,
                actual_value: actual,
                absolute_error_percentage: None,
                error: None,
            };
            replays.push(match outcome {
                Ok(predicted) => BacktestReplay {
                    predicted_value: Some(predicted),
                    absolute_error_percentage: Some(js_round((predicted - actual).abs() / actual * 1_000.0) / 10.0),
                    ..base
                },
                Err(error) => BacktestReplay { error: Some(error.to_string()), ..base },
            });
        }
    }
    let summary = options
        .models
        .iter()
        .map(|model| {
            let rows: Vec<&BacktestReplay> = replays.iter().filter(|row| &row.model == model).collect();
            let completed: Vec<f64> = rows.iter().filter_map(|row| row.absolute_error_percentage).collect();
            BacktestModelSummary {
                model: model.clone(),
                completed: completed.len(),
                failed: rows.len() - completed.len(),
                median_absolute_error_percentage: median(&completed),
            }
        })
        .collect();
    Ok(BacktestReport {
        athleticstandard_version: ATHLETIC_STANDARD_VERSION,
        created_at: options.created_at.clone().unwrap_or_else(time::now_utc),
        models: options.models.clone(),
        replayable_result_count: targets.len(),
        skipped_first_result_count: skipped_first,
        replays,
        summary,
    })
}

/// `backtest-<created_at without : and .>.json` beside the database.
pub fn report_path(database_path: &Path, created_at: &str) -> PathBuf {
    let stamp: String = created_at.chars().filter(|character| *character != ':' && *character != '.').collect();
    database_path.parent().unwrap_or_else(|| Path::new(".")).join(format!("backtest-{stamp}.json"))
}

/// Write a report as a new owner-only file beside the database. An existing
/// file is never replaced.
pub fn write_backtest_report(database_path: &Path, report: &BacktestReport) -> Result<PathBuf> {
    let path = report_path(database_path, &report.created_at);
    let mut text = json::to_pretty(&json::to_value(report));
    text.push('\n');
    crate::files::write_new_synced(&path, text.as_bytes())?;
    Ok(path)
}

/// The `ath backtest --json` object: the report path, then the report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestOutput {
    pub report: String,
    #[serde(flatten)]
    pub body: BacktestReport,
}

/// The human lines `ath backtest` printed, without the final `wrote` line.
pub fn render_backtest(report: &BacktestReport) -> String {
    let models = report.models.len();
    let mut lines = vec![format!(
        "replayed {} results with {models} model{}",
        report.replayable_result_count,
        if models == 1 { "" } else { "s" }
    )];
    for summary in &report.summary {
        lines.push(format!(
            "  {}: {} completed, {} failed, median absolute error {}%",
            summary.model,
            summary.completed,
            summary.failed,
            summary.median_absolute_error_percentage.map(json::format_number).unwrap_or_else(|| "—".into())
        ));
    }
    lines.join("\n")
}
