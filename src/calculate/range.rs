//! `ath calculate`: the eight named range calculations.
//!
//! Each calculation reads typed rows, never the complete stored JSON, except
//! for summaries a session reported, which live only in its JSON. Rules:
//!
//! - Manual sources never enter a device calculation.
//! - Sources are never pooled. Each result is per source, or names the two
//!   sources it compares.
//! - One request names one quantity, so RMSSD and SDNN, the temperature
//!   signals, and the distance types stay apart. Mixed units are refused.
//! - A provider period is never turned into a calendar day.
//! - More than 100,000 inputs is refused. The receipt lists at most 1,000
//!   input references and says when it was shortened; the arithmetic always
//!   uses every accepted input.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use super::definition::shipped_method;
use super::record::parse_millis;
use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::{point_measurement_unit, series_quantity_unit, EvidenceReference, PointMeasurementType};
use crate::model::{ProviderReportingPeriodType, SeriesQuantity};
use crate::view::calendar::{resolve_calendar_range_in, CalendarRangeRequest};

pub const CALCULATION_VERSION: &str = "ath-runtime-v1";
/// A calculation that matches more inputs than this is refused.
pub const MAX_RUNTIME_INPUTS: usize = 100_000;
/// A receipt lists at most this many input references.
pub const MAX_RECEIPT_REFERENCES: usize = 1_000;

macro_rules! named_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident = $text:literal),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),* }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text { $($text => Some($name::$variant),)* _ => None }
            }

            /// Every name, joined with `, `.
            pub fn names() -> String {
                Self::ALL.iter().map(|item| item.as_str()).collect::<Vec<_>>().join(", ")
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
    };
}

named_enum! {
    /// A named range calculation.
    pub enum Operation {
        Summary = "summary",
        Baseline = "baseline",
        Trend = "trend",
        Coverage = "coverage",
        CompareSources = "compare-sources",
        Correlation = "correlation",
        WorkoutDrift = "workout-drift",
        ProviderPeriodSummary = "provider-period-summary",
    }
}

named_enum! {
    /// Which typed rows a calculation reads.
    pub enum RecordKind {
        Measurement = "measurement",
        ProviderScore = "provider-score",
        ProviderSummary = "provider-summary",
        SessionSummary = "session-summary",
    }
}

named_enum! {
    /// Calendar grouping for summary and baseline.
    pub enum Group {
        None = "none",
        Day = "day",
        Week = "week",
        Month = "month",
    }
}

/// A range calculation request.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculateOptions {
    pub operation: Operation,
    /// A measurement quantity, provider metric, or summary type.
    pub quantity: String,
    pub record_kind: RecordKind,
    pub provider_period_type: Option<ProviderReportingPeriodType>,
    pub source_id: Option<String>,
    pub compare_source_id: Option<String>,
    pub other_quantity: Option<String>,
    pub workout_session_id: Option<String>,
    /// A calendar day (`YYYY-MM-DD`) or an exact UTC instant.
    pub from: Option<String>,
    pub to: Option<String>,
    pub time_zone: Option<String>,
    pub group: Group,
}

impl CalculateOptions {
    pub fn new(operation: Operation, quantity: impl Into<String>) -> Self {
        CalculateOptions {
            operation,
            quantity: quantity.into(),
            record_kind: RecordKind::Measurement,
            provider_period_type: None,
            source_id: None,
            compare_source_id: None,
            other_quantity: None,
            workout_session_id: None,
            from: None,
            to: None,
            time_zone: None,
            group: Group::None,
        }
    }
}

/// The window a receipt reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReceiptRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

/// The records a calculation used.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReceiptInput {
    pub record_count: usize,
    /// The exact record revisions used, in input order, at most 1,000.
    pub record_references: Vec<EvidenceReference>,
    /// True when `record_references` was shortened to the receipt limit.
    pub references_truncated: bool,
}

/// Count, minimum, maximum, mean, sum, and population standard deviation.
/// The mean, sum, and deviation are rounded to four decimal places.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NumericSummary {
    pub count: usize,
    pub minimum: f64,
    pub maximum: f64,
    pub mean: f64,
    pub sum: f64,
    pub standard_deviation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupSummary {
    pub period: String,
    #[serde(flatten)]
    pub summary: NumericSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SourceSummary {
    Whole {
        source_id: String,
        #[serde(flatten)]
        summary: NumericSummary,
    },
    Grouped {
        source_id: String,
        groups: Vec<GroupSummary>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceTrend {
    pub source_id: String,
    pub count: usize,
    pub slope_per_day: f64,
    pub first_value: f64,
    pub last_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceCoverage {
    pub source_id: String,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub days_expected: usize,
    pub days_present: usize,
    pub missing_dates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NamedSummary {
    pub source_id: String,
    #[serde(flatten)]
    pub summary: NumericSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompareSourcesResult {
    pub source: NamedSummary,
    pub comparison_source: NamedSummary,
    pub mean_difference: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrelationResult {
    pub other_quantity: String,
    pub paired_day_count: usize,
    /// Null when fewer than two days pair or a side does not vary.
    pub correlation: Option<f64>,
    pub paired_dates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SourceDrift {
    Available {
        source_id: String,
        available: bool,
        first_half_mean: f64,
        second_half_mean: f64,
        change: f64,
        change_percentage: f64,
    },
    Unavailable {
        source_id: String,
        available: bool,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkoutDriftResult {
    pub workout_session_id: String,
    pub sources: Vec<SourceDrift>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeriodSummary {
    /// The provider period exactly as stored.
    pub provider_period: Value,
    #[serde(flatten)]
    pub summary: NumericSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourcePeriods {
    pub source_id: String,
    pub periods: Vec<PeriodSummary>,
}

/// The operation-specific part of a receipt.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CalculationResult {
    Summaries(Vec<SourceSummary>),
    Trends(Vec<SourceTrend>),
    Coverage(Vec<SourceCoverage>),
    CompareSources(CompareSourcesResult),
    Correlation(CorrelationResult),
    WorkoutDrift(WorkoutDriftResult),
    ProviderPeriods(Vec<SourcePeriods>),
}

/// The `ath calculate --json` receipt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalculationReceipt {
    pub calculation_version: &'static str,
    pub operation: Operation,
    pub record_kind: RecordKind,
    pub method: String,
    pub quantity: String,
    pub unit: String,
    pub range: ReceiptRange,
    pub sources: Vec<String>,
    pub input: ReceiptInput,
    pub exclusions: Vec<String>,
    pub result: CalculationResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_period_type: Option<ProviderReportingPeriodType>,
}

/// One accepted input row.
#[derive(Debug, Clone)]
struct Input {
    record_id: String,
    revision: u64,
    source_id: String,
    recorded_at: String,
    value: f64,
    unit: String,
    period: Option<Value>,
}

/// The fully resolved request the arithmetic runs on.
struct Request<'a> {
    options: &'a CalculateOptions,
    from: Option<String>,
    to: Option<String>,
    time_zone: Option<String>,
    expected_unit: Option<&'static str>,
    expected_other_unit: Option<&'static str>,
    method: String,
}

impl Request<'_> {
    fn range(&self) -> ReceiptRange {
        ReceiptRange { from: self.from.clone(), to: self.to.clone(), time_zone: self.time_zone.clone() }
    }

    fn time_zone(&self) -> Result<&str> {
        self.time_zone.as_deref().ok_or_else(|| Error::other("missing time_zone"))
    }

    fn method_or(&self, fallback: &str) -> String {
        if self.method.is_empty() {
            fallback.to_string()
        } else {
            self.method.clone()
        }
    }
}

/// The standard unit of a point or series quantity, or the published refusal.
pub fn known_measurement_unit(quantity: &str) -> Result<&'static str> {
    point_measurement_unit(quantity).or_else(|| series_quantity_unit(quantity)).ok_or_else(|| {
        let mut names: Vec<&str> = PointMeasurementType::ALL.iter().map(|item| item.as_str()).collect();
        for item in SeriesQuantity::ALL {
            if !names.contains(&item.as_str()) {
                names.push(item.as_str());
            }
        }
        Error::other(format!("Unknown quantity '{quantity}'. Known quantities: {}", names.join(", ")))
    })
}

/// Run a named range calculation. Reading never writes athlete data.
/// Outside the library, use `calculate::calculate_range`.
pub(crate) fn calculate(connection: &Connection, options: &CalculateOptions) -> Result<CalculationReceipt> {
    let operation = options.operation;
    let kind = options.record_kind;
    let period_summary = operation == Operation::ProviderPeriodSummary;
    if !period_summary && kind == RecordKind::SessionSummary {
        return Err(Error::other("session-summary records require the provider-period-summary operation"));
    }
    if period_summary && options.provider_period_type.is_none() {
        return Err(Error::other("provider-period-summary needs --period-type <provider-period-type>"));
    }
    if options.provider_period_type.is_some() && !period_summary {
        return Err(Error::other("--period-type is used only with provider-period-summary"));
    }
    if operation == Operation::WorkoutDrift && options.workout_session_id.is_none() {
        return Err(Error::other("workout-drift needs --session <workout-session-id>"));
    }
    if operation == Operation::CompareSources && (options.source_id.is_none() || options.compare_source_id.is_none()) {
        return Err(Error::other("compare-sources needs --source and --compare-source"));
    }
    if operation == Operation::Correlation && options.other_quantity.is_none() {
        return Err(Error::other("correlation needs --other-quantity"));
    }
    let needs_calendar = !period_summary
        && operation != Operation::WorkoutDrift
        && (options.group != Group::None || operation == Operation::Coverage || operation == Operation::Correlation);
    let (from, to, time_zone) = if needs_calendar {
        let range = resolve_calendar_range_in(
            connection,
            &CalendarRangeRequest {
                from: options.from.clone(),
                to: options.to.clone(),
                time_zone: options.time_zone.clone(),
                raw: false,
            },
        )?;
        (range.from, range.to, range.time_zone.or_else(|| options.time_zone.clone()))
    } else {
        // Without calendar days the bounds are compared as given.
        (options.from.clone(), options.to.clone(), options.time_zone.clone())
    };
    let expected_unit = if !period_summary && kind == RecordKind::Measurement {
        Some(known_measurement_unit(&options.quantity)?)
    } else {
        None
    };
    let expected_other_unit = match &options.other_quantity {
        Some(other) if kind == RecordKind::Measurement => Some(known_measurement_unit(other)?),
        _ => None,
    };
    let request = Request {
        options,
        from,
        to,
        time_zone,
        expected_unit,
        expected_other_unit,
        method: shipped_method(operation.as_str()),
    };
    run(connection, &request)
}

fn run(connection: &Connection, request: &Request<'_>) -> Result<CalculationReceipt> {
    let options = request.options;
    let quantity = options.quantity.as_str();
    let kind = options.record_kind;
    if let Some(period_type) = options.provider_period_type {
        let inputs = read_period(connection, request, quantity, period_type)?;
        if inputs.is_empty() {
            return Err(Error::other(format!(
                "No current {} {quantity} records matched {}",
                kind.as_str(),
                period_type.as_str()
            )));
        }
        let unit = one_unit(&inputs, None)?;
        let method = request.method_or(
            "count, minimum, maximum, mean, and sum per exact provider period and source; no calendar boundary conversion",
        );
        return Ok(receipt(
            request,
            unit,
            &inputs,
            request.range(),
            method,
            CalculationResult::ProviderPeriods(period_summaries(&inputs)),
            &[
                "manual sources excluded",
                &format!("records without {} excluded", period_type.as_str()),
                "provider periods were not converted to calendar days",
            ],
        ));
    }
    if options.operation == Operation::WorkoutDrift {
        return workout_drift(connection, request);
    }
    let inputs = read_inputs(connection, request, quantity, options.source_id.as_deref(), None)?;
    if inputs.is_empty() {
        return Err(Error::other(format!(
            "No current {} {quantity} records with an instant matched. Date-only records require provider-period-summary.",
            kind.as_str()
        )));
    }
    let unit = one_unit(&inputs, request.expected_unit)?;
    let manual = ["manual sources excluded"];
    match options.operation {
        Operation::Summary | Operation::Baseline => {
            let mut result = Vec::new();
            for (source_id, values) in by_source(&inputs) {
                if options.group == Group::None {
                    let numbers: Vec<f64> = values.iter().map(|input| input.value).collect();
                    result.push(SourceSummary::Whole { source_id, summary: summarize(&numbers) });
                } else {
                    result.push(SourceSummary::Grouped {
                        source_id,
                        groups: grouped(&values, options.group, request.time_zone()?)?,
                    });
                }
            }
            let method = if options.operation == Operation::Baseline {
                request.method_or("population mean and population standard deviation per source")
            } else if options.group == Group::None {
                request.method_or("count, minimum, maximum, mean, and sum per source")
            } else {
                format!("{} calendar groups per source", options.group.as_str())
            };
            Ok(receipt(request, unit, &inputs, request.range(), method, CalculationResult::Summaries(result), &manual))
        }
        Operation::Trend => {
            let result = by_source(&inputs)
                .into_iter()
                .map(|(source_id, values)| trend(source_id, &values))
                .collect::<Result<Vec<_>>>()?;
            let method = request
                .method_or("ordinary least-squares slope against elapsed days, calculated separately per source");
            Ok(receipt(request, unit, &inputs, request.range(), method, CalculationResult::Trends(result), &manual))
        }
        Operation::Coverage => coverage(request, unit, &inputs),
        Operation::CompareSources => compare_sources(connection, request, unit, inputs),
        Operation::Correlation => correlate(connection, request, unit, inputs),
        Operation::WorkoutDrift | Operation::ProviderPeriodSummary => {
            Err(Error::other(format!("Unsupported calculation operation: {}", options.operation.as_str())))
        }
    }
}

fn workout_drift(connection: &Connection, request: &Request<'_>) -> Result<CalculationReceipt> {
    let options = request.options;
    let session_id = options.workout_session_id.clone().unwrap_or_default();
    let not_a_session = || Error::other(format!("{session_id} is not a current workout session"));
    let row: Option<(String, String, i64)> = connection
        .query_row(
            "SELECT records.record_type, record_revisions.record_json, records.is_deleted
             FROM records
             JOIN record_revisions ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((record_type, record_json, deleted)) = row else {
        return Err(not_a_session());
    };
    if deleted != 0 || record_type != "workout_session" {
        return Err(not_a_session());
    }
    let data = json::parse(&record_json)?;
    let start = data.get("start_at").and_then(Value::as_str).unwrap_or("").to_string();
    let end = data.get("end_at").and_then(Value::as_str).unwrap_or("").to_string();
    let window = (Some(start.clone()), Some(end.clone()));
    let inputs = read_inputs(connection, request, &options.quantity, options.source_id.as_deref(), Some(window))?;
    if inputs.len() < 2 {
        return Err(Error::other("workout-drift needs at least two samples inside the session"));
    }
    let unit = one_unit(&inputs, request.expected_unit)?;
    let start_ms = parse_ms(&start);
    let end_ms = parse_ms(&end);
    let midpoint = start_ms + (end_ms - start_ms) / 2;
    let sources = by_source(&inputs)
        .into_iter()
        .map(|(source_id, values)| {
            let first: Vec<f64> = values
                .iter()
                .filter(|input| parse_ms(&input.recorded_at) < midpoint)
                .map(|input| input.value)
                .collect();
            let second: Vec<f64> = values
                .iter()
                .filter(|input| parse_ms(&input.recorded_at) >= midpoint)
                .map(|input| input.value)
                .collect();
            if first.is_empty() || second.is_empty() {
                return SourceDrift::Unavailable { source_id, available: false, reason: "one half has no samples" };
            }
            let first_mean = summarize(&first).mean;
            let second_mean = summarize(&second).mean;
            SourceDrift::Available {
                source_id,
                available: true,
                first_half_mean: first_mean,
                second_half_mean: second_mean,
                change: round4(second_mean - first_mean),
                change_percentage: round4(((second_mean - first_mean) / first_mean) * 100.0),
            }
        })
        .collect();
    let range = ReceiptRange { from: Some(start), to: Some(end), time_zone: None };
    let method = request.method_or("difference between means in equal-duration first and second workout halves");
    Ok(receipt(
        request,
        unit,
        &inputs,
        range,
        method,
        CalculationResult::WorkoutDrift(WorkoutDriftResult { workout_session_id: session_id, sources }),
        &["manual sources excluded", "samples outside the workout session excluded"],
    ))
}

fn coverage(request: &Request<'_>, unit: String, inputs: &[Input]) -> Result<CalculationReceipt> {
    let zone = request.time_zone()?;
    let options = request.options;
    let first_ten = |value: &Option<String>| value.as_ref().map(|text| text.chars().take(10).collect::<String>());
    let from = first_ten(&options.from.clone().or_else(|| request.from.clone()));
    let to = first_ten(&options.to.clone().or_else(|| request.to.clone()));
    let result = by_source(inputs)
        .into_iter()
        .map(|(source_id, values)| {
            let mut present =
                values.iter().map(|input| calendar_date(&input.recorded_at, zone)).collect::<Result<Vec<_>>>()?;
            present.sort();
            present.dedup();
            let start = from.clone().unwrap_or_else(|| present.first().cloned().unwrap_or_default());
            let end = to.clone().unwrap_or_else(|| present.last().cloned().unwrap_or_default());
            let expected = calendar_dates(&start, &end);
            let present_set: BTreeSet<&String> = present.iter().collect();
            let missing = expected.iter().filter(|date| !present_set.contains(date)).cloned().collect();
            Ok(SourceCoverage {
                source_id,
                first_date: expected.first().cloned(),
                last_date: expected.last().cloned(),
                days_expected: expected.len(),
                days_present: present.len(),
                missing_dates: missing,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(receipt(
        request,
        unit,
        inputs,
        request.range(),
        "calendar dates containing at least one record, calculated separately per source".into(),
        CalculationResult::Coverage(result),
        &["manual sources excluded"],
    ))
}

fn compare_sources(
    connection: &Connection,
    request: &Request<'_>,
    unit: String,
    mut inputs: Vec<Input>,
) -> Result<CalculationReceipt> {
    let options = request.options;
    let source_id = options.source_id.clone().unwrap_or_default();
    let compare_id = options.compare_source_id.clone().unwrap_or_default();
    let other = read_inputs(connection, request, &options.quantity, Some(&compare_id), None)?;
    if other.is_empty() {
        return Err(Error::other(format!("No {} records matched --compare-source", options.quantity)));
    }
    one_unit(&other, Some(&unit)).or_else(|_| one_unit(&other, request.expected_unit))?;
    let left = summarize(&inputs.iter().map(|input| input.value).collect::<Vec<_>>());
    let right = summarize(&other.iter().map(|input| input.value).collect::<Vec<_>>());
    inputs.extend(other);
    let difference = round4(left.mean - right.mean);
    Ok(receipt(
        request,
        unit,
        &inputs,
        request.range(),
        "separate source summaries and difference between their means".into(),
        CalculationResult::CompareSources(CompareSourcesResult {
            source: NamedSummary { source_id, summary: left },
            comparison_source: NamedSummary { source_id: compare_id, summary: right },
            mean_difference: difference,
        }),
        &["manual sources excluded", "records are not paired by timestamp"],
    ))
}

fn correlate(
    connection: &Connection,
    request: &Request<'_>,
    unit: String,
    mut inputs: Vec<Input>,
) -> Result<CalculationReceipt> {
    let options = request.options;
    let other_quantity = options.other_quantity.clone().unwrap_or_default();
    let zone = request.time_zone()?;
    let other_source = options.compare_source_id.as_deref().or(options.source_id.as_deref());
    let other = read_inputs(connection, request, &other_quantity, other_source, None)?;
    one_unit(&other, request.expected_other_unit)?;
    let left = daily_means(&inputs, zone)?;
    let right = daily_means(&other, zone)?;
    let dates: Vec<String> = left.keys().filter(|date| right.contains_key(*date)).cloned().collect();
    inputs.extend(other);
    let correlation = pearson(
        &dates.iter().map(|date| left[date]).collect::<Vec<_>>(),
        &dates.iter().map(|date| right[date]).collect::<Vec<_>>(),
    );
    Ok(receipt(
        request,
        unit,
        &inputs,
        request.range(),
        "Pearson correlation of same-calendar-day means".into(),
        CalculationResult::Correlation(CorrelationResult {
            other_quantity,
            paired_day_count: dates.len(),
            correlation,
            paired_dates: dates,
        }),
        &["manual sources excluded", "days missing either quantity excluded"],
    ))
}

/// One branch of the union: a typed table's rows for a quantity, with the
/// source and window filters inside the branch so each uses its lookup index.
fn branch(
    table: &str,
    quantity_column: &str,
    require_instant: bool,
    source: Option<&str>,
    window: (&Option<String>, &Option<String>),
    args: &mut Vec<Sql>,
    quantity: &str,
) -> String {
    let mut conditions = vec![format!("{table}.{quantity_column} = ?")];
    args.push(Sql::Text(quantity.to_string()));
    if require_instant {
        conditions.push(format!("{table}.recorded_at IS NOT NULL"));
    }
    if let Some(source) = source {
        conditions.push(format!("{table}.source_id = ?"));
        args.push(Sql::Text(source.to_string()));
    }
    if let Some(from) = window.0 {
        conditions.push(format!("{table}.recorded_at >= ?"));
        args.push(Sql::Text(from.clone()));
    }
    if let Some(to) = window.1 {
        conditions.push(format!("{table}.recorded_at <= ?"));
        args.push(Sql::Text(to.clone()));
    }
    format!(
        "SELECT {table}.record_id, {table}.source_id, {table}.recorded_at, {table}.standard_value, {table}.standard_unit
         FROM {table} WHERE {}",
        conditions.join(" AND ")
    )
}

/// Current, non-manual rows with an instant, ordered by source, time, and ID.
fn read_inputs(
    connection: &Connection,
    request: &Request<'_>,
    quantity: &str,
    source: Option<&str>,
    window: Option<(Option<String>, Option<String>)>,
) -> Result<Vec<Input>> {
    let (sql, args) = inputs_sql(request, quantity, source, window);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(args), |row| {
            Ok(Input {
                record_id: row.get(0)?,
                revision: row.get::<_, i64>(1)? as u64,
                source_id: row.get(2)?,
                recorded_at: row.get(3)?,
                value: row.get(4)?,
                unit: row.get(5)?,
                period: None,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    refuse_too_many(rows.len())?;
    crate::package::failpoint!("calculate.after_inputs");
    Ok(rows)
}

/// The SQL and arguments that read instant inputs. Only typed columns are
/// read, and each union branch filters inside itself so it can use its
/// lookup index; no stored record JSON is scanned.
fn inputs_sql(
    request: &Request<'_>,
    quantity: &str,
    source: Option<&str>,
    window: Option<(Option<String>, Option<String>)>,
) -> (String, Vec<Sql>) {
    let (from, to) = window.unwrap_or_else(|| (request.from.clone(), request.to.clone()));
    let bounds = (&from, &to);
    let mut args = Vec::new();
    let values_sql = match request.options.record_kind {
        RecordKind::ProviderScore => branch("provider_scores", "metric", true, source, bounds, &mut args, quantity),
        RecordKind::ProviderSummary => {
            branch("provider_summaries", "summary_type", true, source, bounds, &mut args, quantity)
        }
        _ => {
            let points = branch("point_measurements", "quantity", true, source, bounds, &mut args, quantity);
            let samples = branch("measurement_samples", "quantity", false, source, bounds, &mut args, quantity);
            format!("{points} UNION ALL {samples}")
        }
    };
    args.push(Sql::Integer(MAX_RUNTIME_INPUTS as i64 + 1));
    let sql = format!(
        "SELECT values_by_record.record_id, records.current_revision, values_by_record.source_id,
                values_by_record.recorded_at, values_by_record.standard_value, values_by_record.standard_unit
         FROM ({values_sql}) AS values_by_record
         JOIN records ON records.record_id = values_by_record.record_id
         JOIN sources ON sources.record_id = values_by_record.source_id
         WHERE records.is_deleted = 0 AND sources.source_type <> 'manual'
         ORDER BY values_by_record.source_id, values_by_record.recorded_at, values_by_record.record_id
         LIMIT ?"
    );
    (sql, args)
}

fn refuse_too_many(count: usize) -> Result<()> {
    if count > MAX_RUNTIME_INPUTS {
        return Err(Error::refused(format!(
            "Calculation matched more than {MAX_RUNTIME_INPUTS} records. Narrow --from and --to."
        )));
    }
    Ok(())
}

/// Current, non-manual rows that carry the requested provider period type.
fn read_period(
    connection: &Connection,
    request: &Request<'_>,
    quantity: &str,
    period_type: ProviderReportingPeriodType,
) -> Result<Vec<Input>> {
    let (values_sql, mut args): (&str, Vec<Sql>) = match request.options.record_kind {
        RecordKind::ProviderScore => (
            "SELECT provider_scores.record_id, provider_scores.source_id, provider_scores.standard_value, provider_scores.standard_unit
             FROM provider_scores WHERE provider_scores.metric = ?",
            vec![Sql::Text(quantity.into())],
        ),
        RecordKind::ProviderSummary => (
            "SELECT provider_summaries.record_id, provider_summaries.source_id, provider_summaries.standard_value, provider_summaries.standard_unit
             FROM provider_summaries WHERE provider_summaries.summary_type = ?",
            vec![Sql::Text(quantity.into())],
        ),
        RecordKind::SessionSummary => (
            "SELECT sleep_sessions.record_id, sleep_sessions.source_id,
                    CAST(json_extract(summary.value, '$.standard_value') AS REAL) AS standard_value,
                    CAST(json_extract(summary.value, '$.standard_unit') AS TEXT) AS standard_unit
             FROM sleep_sessions JOIN json_each(sleep_sessions.data_json, '$.source_reported_summaries') AS summary
             WHERE json_extract(summary.value, '$.summary_type') = ?
             UNION ALL
             SELECT workout_sessions.record_id, workout_sessions.source_id,
                    CAST(json_extract(summary.value, '$.standard_value') AS REAL),
                    CAST(json_extract(summary.value, '$.standard_unit') AS TEXT)
             FROM workout_sessions JOIN json_each(workout_sessions.data_json, '$.source_reported_summaries') AS summary
             WHERE json_extract(summary.value, '$.summary_type') = ?",
            vec![Sql::Text(quantity.into()), Sql::Text(quantity.into())],
        ),
        RecordKind::Measurement => (
            "SELECT point_measurements.record_id, point_measurements.source_id, point_measurements.standard_value, point_measurements.standard_unit
             FROM point_measurements WHERE point_measurements.quantity = ?
             UNION ALL
             SELECT measurement_samples.record_id, measurement_samples.source_id, measurement_samples.standard_value, measurement_samples.standard_unit
             FROM measurement_samples WHERE measurement_samples.quantity = ?",
            vec![Sql::Text(quantity.into()), Sql::Text(quantity.into())],
        ),
    };
    let mut conditions = vec![
        "records.is_deleted = 0".to_string(),
        "sources.source_type <> 'manual'".to_string(),
        "provider_reporting_periods.period_type = ?".to_string(),
    ];
    args.push(Sql::Text(period_type.as_str().into()));
    if let Some(source) = &request.options.source_id {
        conditions.push("values_by_record.source_id = ?".into());
        args.push(Sql::Text(source.clone()));
    }
    let period_key = "COALESCE(provider_reporting_periods.provider_date, provider_reporting_periods.start_at)";
    if let Some(from) = &request.from {
        conditions.push(format!("{period_key} >= ?"));
        args.push(Sql::Text(from.clone()));
    }
    if let Some(to) = &request.to {
        conditions.push(format!("{period_key} <= ?"));
        args.push(Sql::Text(to.clone()));
    }
    args.push(Sql::Integer(MAX_RUNTIME_INPUTS as i64 + 1));
    let sql = format!(
        "WITH values_by_record AS ({values_sql})
         SELECT values_by_record.record_id, records.current_revision, values_by_record.source_id,
                values_by_record.standard_value, values_by_record.standard_unit, provider_reporting_periods.period_json
         FROM values_by_record
         JOIN records ON records.record_id = values_by_record.record_id
         JOIN sources ON sources.record_id = values_by_record.source_id
         JOIN provider_reporting_periods ON provider_reporting_periods.owner_record_id = values_by_record.record_id
         WHERE {}
         ORDER BY values_by_record.source_id, {period_key}, values_by_record.record_id
         LIMIT ?",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(args), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    refuse_too_many(rows.len())?;
    rows.into_iter()
        .map(|(record_id, revision, source_id, value, unit, period_json)| {
            let period = json::parse(&period_json)?;
            let recorded_at = period_sort(&period);
            Ok(Input {
                record_id,
                revision: revision as u64,
                source_id,
                recorded_at,
                value,
                unit,
                period: Some(period),
            })
        })
        .collect()
}

fn summarize(values: &[f64]) -> NumericSummary {
    let count = values.len() as f64;
    let sum = values.iter().sum::<f64>();
    let mean = sum / count;
    let variance = values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / count;
    NumericSummary {
        count: values.len(),
        minimum: values.iter().copied().fold(f64::INFINITY, f64::min),
        maximum: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        mean: round4(mean),
        sum: round4(sum),
        standard_deviation: round4(variance.sqrt()),
    }
}

fn trend(source_id: String, inputs: &[&Input]) -> Result<SourceTrend> {
    if inputs.len() < 2 {
        return Err(Error::other("A trend needs at least two inputs"));
    }
    let start = parse_ms(&inputs[0].recorded_at);
    let points: Vec<(f64, f64)> = inputs
        .iter()
        .map(|input| ((parse_ms(&input.recorded_at) - start) as f64 / 86_400_000.0, input.value))
        .collect();
    let x_mean = points.iter().map(|point| point.0).sum::<f64>() / points.len() as f64;
    let y_mean = points.iter().map(|point| point.1).sum::<f64>() / points.len() as f64;
    let numerator = points.iter().map(|point| (point.0 - x_mean) * (point.1 - y_mean)).sum::<f64>();
    let denominator = points.iter().map(|point| (point.0 - x_mean).powi(2)).sum::<f64>();
    if denominator == 0.0 {
        return Err(Error::other("A trend needs inputs at more than one instant"));
    }
    Ok(SourceTrend {
        source_id,
        count: points.len(),
        slope_per_day: round4(numerator / denominator),
        first_value: inputs[0].value,
        last_value: inputs[inputs.len() - 1].value,
    })
}

/// A period's grouping key, the stored period, and its values.
type PeriodGroup = (String, Value, Vec<f64>);

fn period_summaries(inputs: &[Input]) -> Vec<SourcePeriods> {
    let mut sources: Vec<(String, Vec<PeriodGroup>)> = Vec::new();
    for input in inputs {
        let period = input.period.clone().unwrap_or(Value::Null);
        let field = |name: &str| period.get(name).cloned().unwrap_or(Value::Null);
        let key = json::to_string(&Value::Array(vec![
            field("provider_period_id"),
            field("provider_date"),
            field("start_at"),
            field("end_at"),
        ]));
        match sources.iter_mut().find(|group| group.0 == input.source_id) {
            Some(group) => match group.1.iter_mut().find(|item| item.0 == key) {
                Some(period_group) => period_group.2.push(input.value),
                None => group.1.push((key, period, vec![input.value])),
            },
            None => sources.push((input.source_id.clone(), vec![(key, period, vec![input.value])])),
        }
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
        .into_iter()
        .map(|(source_id, mut periods)| {
            periods.sort_by_key(|item| period_sort(&item.1));
            SourcePeriods {
                source_id,
                periods: periods
                    .into_iter()
                    .map(|(_, period, values)| PeriodSummary { provider_period: period, summary: summarize(&values) })
                    .collect(),
            }
        })
        .collect()
}

fn by_source(inputs: &[Input]) -> Vec<(String, Vec<&Input>)> {
    let mut groups: Vec<(String, Vec<&Input>)> = Vec::new();
    for input in inputs {
        match groups.iter_mut().find(|group| group.0 == input.source_id) {
            Some(group) => group.1.push(input),
            None => groups.push((input.source_id.clone(), vec![input])),
        }
    }
    groups
}

fn grouped(inputs: &[&Input], group: Group, zone: &str) -> Result<Vec<GroupSummary>> {
    let mut groups: Vec<(String, Vec<f64>)> = Vec::new();
    for input in inputs {
        let date = calendar_date(&input.recorded_at, zone)?;
        let key = match group {
            Group::Day => date,
            Group::Week => start_of_week(&date),
            Group::Month => date.chars().take(7).collect(),
            Group::None => "all".into(),
        };
        match groups.iter_mut().find(|item| item.0 == key) {
            Some(existing) => existing.1.push(input.value),
            None => groups.push((key, vec![input.value])),
        }
    }
    groups.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(groups.into_iter().map(|(period, values)| GroupSummary { period, summary: summarize(&values) }).collect())
}

fn daily_means(inputs: &[Input], zone: &str) -> Result<BTreeMap<String, f64>> {
    let mut days: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for input in inputs {
        days.entry(calendar_date(&input.recorded_at, zone)?).or_default().push(input.value);
    }
    Ok(days.into_iter().map(|(date, values)| (date, values.iter().sum::<f64>() / values.len() as f64)).collect())
}

fn pearson(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;
    let mut numerator = 0.0;
    let mut left_squares = 0.0;
    let mut right_squares = 0.0;
    for index in 0..left.len() {
        let left_difference = left[index] - left_mean;
        let right_difference = right[index] - right_mean;
        numerator += left_difference * right_difference;
        left_squares += left_difference.powi(2);
        right_squares += right_difference.powi(2);
    }
    let denominator = (left_squares * right_squares).sqrt();
    if denominator == 0.0 {
        None
    } else {
        Some(round4(numerator / denominator))
    }
}

fn receipt(
    request: &Request<'_>,
    unit: String,
    inputs: &[Input],
    range: ReceiptRange,
    method: String,
    result: CalculationResult,
    exclusions: &[&str],
) -> CalculationReceipt {
    let mut sources: Vec<String> = inputs.iter().map(|input| input.source_id.clone()).collect();
    sources.sort();
    sources.dedup();
    let record_references = inputs
        .iter()
        .take(MAX_RECEIPT_REFERENCES)
        .map(|input| EvidenceReference { record_id: input.record_id.clone(), revision: input.revision })
        .collect();
    CalculationReceipt {
        calculation_version: CALCULATION_VERSION,
        operation: request.options.operation,
        record_kind: request.options.record_kind,
        method,
        quantity: request.options.quantity.clone(),
        unit,
        range,
        sources,
        input: ReceiptInput {
            record_count: inputs.len(),
            record_references,
            references_truncated: inputs.len() > MAX_RECEIPT_REFERENCES,
        },
        exclusions: exclusions.iter().map(|text| text.to_string()).collect(),
        result,
        provider_period_type: request.options.provider_period_type,
    }
}

fn one_unit(inputs: &[Input], expected: Option<&str>) -> Result<String> {
    let mut units: Vec<&str> = inputs.iter().map(|input| input.unit.as_str()).collect();
    units.sort_unstable();
    units.dedup();
    if units.len() != 1 {
        // With no units the 2.1.0 message ended at the colon.
        let message = format!("Calculation matched incompatible units: {}", units.join(", "));
        return Err(Error::refused(message.trim_end()));
    }
    if let Some(expected) = expected {
        if units[0] != expected {
            return Err(Error::refused(format!("Calculation expected {expected} but matched {}", units[0])));
        }
    }
    Ok(units[0].to_string())
}

/// The local date of an instant. The 2.1.0 range arithmetic read the
/// timezone name exactly, so a name in another letter case is refused here.
fn calendar_date(timestamp: &str, zone: &str) -> Result<String> {
    let tz: Tz = zone.parse().map_err(|_| crate::view::calendar::not_a_time_zone(zone))?;
    let instant = DateTime::parse_from_rfc3339(timestamp).map_err(|error| Error::other(error.to_string()))?;
    Ok(instant.with_timezone(&tz).format("%Y-%m-%d").to_string())
}

fn calendar_dates(from: &str, to: &str) -> Vec<String> {
    let part = |text: &str, range: std::ops::Range<usize>, default: u32| {
        text.get(range).and_then(|value| value.parse::<u32>().ok()).unwrap_or(default)
    };
    let date = |text: &str| {
        Utc.with_ymd_and_hms(part(text, 0..4, 1970) as i32, part(text, 5..7, 1), part(text, 8..10, 1), 0, 0, 0).single()
    };
    let mut current = date(from).unwrap_or_else(Utc::now);
    let end = date(to).unwrap_or(current);
    let mut dates = Vec::new();
    while current <= end {
        dates.push(current.format("%Y-%m-%d").to_string());
        current += chrono::Duration::days(1);
    }
    dates
}

fn start_of_week(date: &str) -> String {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_default();
    let day = parsed.weekday().num_days_from_monday();
    (parsed - chrono::Duration::days(i64::from(day))).format("%Y-%m-%d").to_string()
}

fn period_sort(period: &Value) -> String {
    ["provider_date", "start_at", "provider_period_id"]
        .iter()
        .find_map(|key| period.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn parse_ms(value: &str) -> i64 {
    parse_millis(value).unwrap_or(0)
}

/// Rounded to four decimal places, halves away from zero.
fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// The human text `ath calculate` printed.
pub fn render_calculation(receipt: &CalculationReceipt) -> String {
    render_calculation_value(&json::to_value(receipt))
}

/// The same text from a receipt's JSON form, which is what the JavaScript
/// `renderCalculation` receives. A missing field prints as JavaScript
/// template text does.
pub fn render_calculation_value(receipt: &Value) -> String {
    fn text(value: Option<&Value>) -> String {
        match value {
            None => "undefined".to_string(),
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => json::format_number(json::number_as_f64(number)),
            Some(Value::Array(items)) => items.iter().map(|item| text(Some(item))).collect::<Vec<_>>().join(","),
            Some(Value::Object(_)) => "[object Object]".to_string(),
            Some(other) => json::to_string(other),
        }
    }
    let field = |key: &str| text(receipt.get(key));
    let mut lines =
        vec![format!("{}: {} {} ({})", field("operation"), field("record_kind"), field("quantity"), field("unit"))];
    if let Some(period_type) = receipt.get("provider_period_type") {
        lines.push(format!("provider period: {}", text(Some(period_type))));
    }
    lines.push(format!("method: {}", field("method")));
    let count = receipt.get("input").and_then(|input| input.get("record_count"));
    let plural = if count.and_then(Value::as_f64) == Some(1.0) { "" } else { "s" };
    lines.push(format!("inputs: {} record{plural}", text(count)));
    let sources = match receipt.get("sources") {
        Some(Value::Array(items)) => items.iter().map(|item| text(Some(item))).collect::<Vec<_>>().join(", "),
        other => text(other),
    };
    lines.push(format!("sources: {sources}"));
    lines.push(match receipt.get("result") {
        Some(result) => json::to_pretty(result),
        None => "undefined".to_string(),
    });
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{CreateOptions, Store};

    fn query_plan(connection: &Connection, sql: &str, args: Vec<Sql>) -> Vec<String> {
        let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        statement
            .query_map(rusqlite::params_from_iter(args), |row| row.get::<_, String>(3))
            .unwrap()
            .map(std::result::Result::unwrap)
            .collect()
    }

    #[test]
    fn bounded_inputs_use_lookup_indexes_and_never_read_stored_json() {
        let directory = crate::files::make_temporary_directory("ath-plan-").unwrap();
        let store = Store::create(directory.join("plan.ath.sqlite"), CreateOptions::default()).unwrap();
        for (kind, quantity, tables) in [
            (RecordKind::Measurement, "hrv_rmssd", vec!["point_measurements", "measurement_samples"]),
            (RecordKind::ProviderScore, "recovery", vec!["provider_scores"]),
            (RecordKind::ProviderSummary, "sleep_need", vec!["provider_summaries"]),
        ] {
            for (source, from, to) in
                [(None, None, None), (Some("s"), Some("2026-01-01T00:00:00.000Z"), Some("2026-02-01T00:00:00.000Z"))]
            {
                let mut options = CalculateOptions::new(Operation::Summary, quantity);
                options.record_kind = kind;
                let request = Request {
                    options: &options,
                    from: from.map(str::to_string),
                    to: to.map(str::to_string),
                    time_zone: None,
                    expected_unit: None,
                    expected_other_unit: None,
                    method: String::new(),
                };
                let (sql, args) = inputs_sql(&request, quantity, source, None);
                let plan = query_plan(store.connection(), &sql, args);
                let text = plan.join("\n");
                for table in &tables {
                    assert!(
                        plan.iter()
                            .any(|step| step.starts_with(&format!("SEARCH {table} USING INDEX {table}_lookup_idx"))),
                        "{table} does not use its lookup index:\n{text}"
                    );
                    assert!(
                        !plan.iter().any(|step| step.starts_with(&format!("SCAN {table}"))),
                        "{table} is scanned:\n{text}"
                    );
                }
                assert!(!text.contains("record_revisions"), "stored JSON is read:\n{text}");
            }
        }
        drop(store);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
