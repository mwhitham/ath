//! Oura's CSV exports: the API-shaped tables and the member-portal tables.
//!
//! Oura names its tables and columns differently between export routes, so
//! every CSV in the export is read and each column is found by its normalized
//! name. A row is placed at its bedtime end when that time carries an offset,
//! and otherwise at its `day`.
//!
//! Oura assigns each value to a day it chose. A sleep value belongs to the day
//! the sleep ended, an activity value to its activity day, and a readiness
//! value to a day Oura does not define. Each value keeps that day as a
//! provider reporting period: `oura_sleep_day`, `oura_activity_day`, or
//! `oura_day_unspecified`. A value with only a day has no invented instant.
//!
//! - Average HRV is stored as `hrv_rmssd` with the `provider_documented_rmssd`
//!   basis, because Oura documents it as RMSSD (D26).
//! - Readiness, sleep, and activity scores, and readiness contributors, are
//!   provider scores on the scale `0-100`, not measurements.
//! - A member-portal score row names its table only through the keys of its
//!   `contributors` JSON. The keys that most resemble readiness, sleep, or
//!   activity contributors choose the metric.
//! - Activity columns, breathing disturbance, and cardiovascular age are not
//!   stored. Each present value is counted as skipped. A `met` cell counts even
//!   when it holds JSON instead of a number.
//!
//! These are the rules of `crates/ath-core/src/oura.rs` and the Oura branch of
//! `src/db/provider-import.ts` in 2.1.0.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::json::{self, Value};
use crate::model::*;

use super::candidate::{
    missing_timestamp_receipt, offset_timestamp_receipt, source_reported_summaries, source_value, value_basis,
    value_field_basis, Aggregate, BasisFacts, Candidate, MatchKey, Section, SourceInput,
};
use super::csv::{self, cell, number, positive, Row};
use super::detect::{self, DetectedExport};
use super::instant::normalized_utc;
use super::provider::{CandidateSink, ProviderReader};
use super::whoop::with_offset;

/// The Oura CSV route.
pub struct OuraReader;

const SLEEP_POINTS: [(&[&str], PointMeasurementType, &str); 5] = [
    (&["average_hrv", "Average HRV", "hrv"], PointMeasurementType::HrvRmssd, "ms"),
    (
        &["lowest_heart_rate", "Lowest Resting Heart Rate", "lowest_resting_heart_rate"],
        PointMeasurementType::RestingHeartRate,
        "bpm",
    ),
    (&["average_breath", "Respiratory Rate", "respiratory_rate"], PointMeasurementType::RespiratoryRate, "brpm"),
    (&["spo2_percentage", "Average SpO2", "spo2"], PointMeasurementType::OxygenSaturation, "%"),
    (&["vo2_max"], PointMeasurementType::Vo2Max, "ml/kg/min"),
];

const SPO2_COLUMNS: [&str; 3] = ["spo2_percentage", "Average SpO2", "spo2"];

const SLEEP_STAGES: [(&[&str], &str); 6] = [
    (&["total_sleep_duration", "Total Sleep Duration"], "duration_s"),
    (&["time_in_bed", "Time in Bed"], "time_in_bed_s"),
    (&["deep_sleep_duration", "Deep Sleep Duration"], "deep_s"),
    (&["rem_sleep_duration", "REM Sleep Duration"], "rem_s"),
    (&["light_sleep_duration", "Light Sleep Duration"], "light_s"),
    (&["awake_time", "Awake Time", "awake_duration"], "awake_s"),
];

const VENDOR_SCORES: [(&[&str], &str); 3] = [
    (&["readiness_score", "Readiness Score"], "readiness"),
    (&["sleep_score", "Sleep Score"], "sleep_score"),
    (&["activity_score", "Activity Score"], "activity_score"),
];

/// Readiness contributors, in the order they are stored.
const CONTRIBUTORS: [&str; 9] = [
    "activity_balance",
    "body_temperature",
    "hrv_balance",
    "previous_day_activity",
    "previous_night",
    "recovery_index",
    "resting_heart_rate",
    "sleep_balance",
    "sleep_regularity",
];

const SLEEP_CONTRIBUTORS: [&str; 7] =
    ["timing", "latency", "deep_sleep", "rem_sleep", "total_sleep", "efficiency", "restfulness"];

const ACTIVITY_CONTRIBUTORS: [&str; 6] =
    ["stay_active", "move_every_hour", "meet_daily_targets", "training_frequency", "training_volume", "recovery_time"];

/// Activity columns this route does not store. Each present value is a skip.
const ACTIVITY_COLUMNS: [&str; 22] = [
    "steps",
    "active_calories",
    "total_calories",
    "target_calories",
    "met_minutes",
    "low_activity_met_minutes",
    "medium_activity_met_minutes",
    "high_activity_met_minutes",
    "equivalent_walking_distance",
    "target_meters",
    "meters_to_target",
    "inactivity_alerts",
    "resting_time",
    "sedentary_met_minutes",
    "low_activity_time",
    "medium_activity_time",
    "high_activity_time",
    "sedentary_time",
    "non_wear_time",
    "average_met_minutes",
    "met",
    "class_5_min",
];

impl ProviderReader for OuraReader {
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()> {
        let files = detect::read_csv_bundle(export)?;
        let source_id = sink.export_source("Oura", "oura", None);
        let mut reader = Reader { sink, source_id };
        for (name, text) in &files {
            let rows = csv::parse_csv(text);
            if rows.is_empty() {
                reader.sink.skip_count(&format!("empty CSV: {name}"), 1);
                continue;
            }
            reader.rows(name, &rows)?;
        }
        Ok(())
    }
}

struct Reader<'s, 'a> {
    sink: &'s mut CandidateSink<'a>,
    source_id: String,
}

/// The provider days one row can carry, by period type.
struct Days {
    sleep: Option<ProviderReportingPeriod>,
    activity: Option<ProviderReportingPeriod>,
    unspecified: Option<ProviderReportingPeriod>,
}

/// Where a value sits in time, before the 2.1.0 placement rule is applied.
struct Placement<'a> {
    /// The row's bedtime end with its offset, or `<day>T00:00:00Z`.
    anchor: &'a str,
    /// The bedtime end as Oura wrote it, when the value claims that time.
    timestamp: Option<&'a str>,
    period: Option<ProviderReportingPeriod>,
}

/// A value's stored time: an instant, a provider day, or both.
struct Placed {
    recorded_at: Option<String>,
    input_timestamp: InputTimestampReceipt,
    periods: Option<Vec<ProviderReportingPeriod>>,
    /// The time part of the match key.
    key_anchor: String,
}

impl Reader<'_, '_> {
    fn rows(&mut self, name: &str, rows: &[Row]) -> Result<()> {
        let mut used = 0;
        let portal = detect_portal(rows);
        for row in rows {
            let bedtime_start_text = cell(row, &["bedtime_start", "Bedtime Start"]);
            let bedtime_end_text = cell(row, &["bedtime_end", "Bedtime End"]);
            let bedtime_start = with_offset(bedtime_start_text.as_deref(), None);
            let bedtime_end = with_offset(bedtime_end_text.as_deref(), None);
            let day = cell(row, &["day", "date", "summary_date"]);
            let days = Days {
                sleep: period(day.as_deref(), ProviderReportingPeriodType::OuraSleepDay),
                activity: period(day.as_deref(), ProviderReportingPeriodType::OuraActivityDay),
                unspecified: period(day.as_deref(), ProviderReportingPeriodType::OuraDayUnspecified),
            };
            let Some(anchor) = bedtime_end.clone().or_else(|| day.as_ref().map(|day| format!("{day}T00:00:00Z")))
            else {
                self.sink.skip_count(&format!("{name}: rows without a date"), 1);
                continue;
            };
            used += 1;
            let end_text = bedtime_end_text.as_deref();

            if let Some(spo2) = cell(row, &SPO2_COLUMNS) {
                if spo2.starts_with('{') {
                    if let Some(value) = json_number(&spo2).filter(|value| *value > 0.0) {
                        let at = Placement { anchor: &anchor, timestamp: None, period: days.unspecified.clone() };
                        self.point(PointMeasurementType::OxygenSaturation, value, "%", at)?;
                    }
                }
            }
            for (columns, quantity, unit) in SLEEP_POINTS {
                if quantity == PointMeasurementType::OxygenSaturation
                    && cell(row, columns).is_some_and(|raw| raw.trim().starts_with('{'))
                {
                    continue;
                }
                let Some(value) = positive(row, columns) else {
                    continue;
                };
                let at = Placement { anchor: &anchor, timestamp: end_text, period: days.sleep.clone() };
                self.point(quantity, value, unit, at)?;
            }
            if let Some(deviation) = number(row, &["temperature_deviation", "Temperature Deviation"]) {
                let at = Placement { anchor: &anchor, timestamp: end_text, period: days.unspecified.clone() };
                self.point(PointMeasurementType::TemperatureDeviation, deviation, "°C", at)?;
            }
            if let (Some(start), Some(end)) = (&bedtime_start, &bedtime_end) {
                if parse_ms(end) > parse_ms(start) {
                    self.sleep(row, (start, bedtime_start_text.as_deref()), (end, end_text), days.sleep.clone())?;
                }
            }

            let score = number(row, &["score", "Score"]);
            let contributors = cell(row, &["contributors"]);
            let keys = contributor_keys(contributors.as_deref());
            if score.is_some() && contributors.is_some() && keys.as_ref().is_some_and(Vec::is_empty) {
                self.sink.skip_count(&format!("{name}: score row with empty contributors"), 1);
            }
            if let (Some(score), Some(metric), Some(keys)) = (score, portal, keys.as_ref()) {
                if contributors.is_some() && !keys.is_empty() {
                    let period = match metric {
                        "sleep_score" => days.sleep.clone(),
                        "activity_score" => days.activity.clone(),
                        _ => days.unspecified.clone(),
                    };
                    self.score(metric, score, Placement { anchor: &anchor, timestamp: None, period })?;
                }
            }
            if number(row, &["breathing_disturbance_index", "breathing_disturbance"]).is_some() {
                self.sink.skip_count(&format!("{name}: breathing disturbance column"), 1);
            }
            if number(row, &["cardiovascular_age"]).is_some() {
                self.sink.skip_count(&format!("{name}: cardiovascular age column"), 1);
            }
            for column in ACTIVITY_COLUMNS {
                let present =
                    if column == "met" { cell(row, &[column]).is_some() } else { number(row, &[column]).is_some() };
                if present {
                    self.sink.skip_count(&format!("{name}: activity column {column}"), 1);
                }
            }
            for (columns, metric) in VENDOR_SCORES {
                let Some(value) = number(row, columns) else {
                    continue;
                };
                let period = match metric {
                    "activity_score" => days.activity.clone(),
                    "sleep_score" => days.sleep.clone(),
                    _ => days.unspecified.clone(),
                };
                self.score(metric, value, Placement { anchor: &anchor, timestamp: end_text, period })?;
            }
            let mut contributor_values: BTreeMap<String, f64> = BTreeMap::new();
            if let Some(text) = contributors.as_deref().filter(|text| text.starts_with('{')) {
                match json::parse(text) {
                    Ok(Value::Object(parsed)) => {
                        for (key, value) in parsed {
                            let Some(number) = value.as_f64() else {
                                continue;
                            };
                            if CONTRIBUTORS.contains(&key.as_str()) {
                                contributor_values.insert(key, number);
                            } else {
                                self.sink.skip_count(&format!("{name}: contributor column {key}"), 1);
                            }
                        }
                    }
                    _ => self.sink.skip_count(&format!("{name}: contributors cell is not valid JSON"), 1),
                }
            }
            for contributor in CONTRIBUTORS {
                let value = contributor_values.get(contributor).copied().or_else(|| {
                    number(row, &[&format!("contributor_{contributor}"), &format!("contributors_{contributor}")])
                });
                let Some(value) = value else {
                    continue;
                };
                let metric = format!("readiness_contributor_{contributor}");
                let at = Placement { anchor: &anchor, timestamp: end_text, period: days.unspecified.clone() };
                self.score(&metric, value, at)?;
            }
        }
        if used == 0 {
            self.sink.skip_count(&format!("{name}: no rows we could place"), 1);
        }
        Ok(())
    }

    fn basis(&self, record_type: RecordType, quantity: Option<&str>) -> FieldBasis {
        value_field_basis(value_basis(BasisFacts {
            data_route: &self.sink.route().data_route,
            record_type,
            quantity,
            derived: false,
            manual: false,
        }))
    }

    fn point(&mut self, quantity: PointMeasurementType, value: f64, unit: &str, at: Placement<'_>) -> Result<()> {
        let placed = place(at)?;
        let data = PointMeasurementData {
            source_id: self.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: placed.periods,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::PointMeasurement, Some(quantity.as_str()))),
            quantity,
            recorded_at: placed.recorded_at,
            input_timestamp: placed.input_timestamp,
            value: source_value(round2(value), unit, Some(&SourceInput::written(value, unit))),
            calculations: None,
        };
        self.sink.add(Candidate::new(
            RecordData::PointMeasurement(data),
            MatchKey::measured(RecordType::PointMeasurement, quantity.as_str(), &placed.key_anchor),
            Section::Record,
        ))
    }

    fn score(&mut self, metric: &str, value: f64, at: Placement<'_>) -> Result<()> {
        let placed = place(at)?;
        let data = ProviderScoreData {
            source_id: self.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: placed.periods,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::ProviderScore, None)),
            metric: metric.to_string(),
            value: source_value(value, "0-100", Some(&SourceInput::without_unit(value))),
            scale: "0-100".to_string(),
            recorded_at: placed.recorded_at,
            input_timestamp: placed.input_timestamp,
        };
        self.sink.add(Candidate::new(
            RecordData::ProviderScore(data),
            MatchKey::measured(RecordType::ProviderScore, metric, &placed.key_anchor),
            Section::Record,
        ))
    }

    fn sleep(
        &mut self,
        row: &Row,
        start: (&str, Option<&str>),
        end: (&str, Option<&str>),
        period: Option<ProviderReportingPeriod>,
    ) -> Result<()> {
        let mut aggregates = Vec::new();
        for (columns, name) in SLEEP_STAGES {
            if let Some(seconds) = positive(row, columns) {
                aggregates.push(Aggregate {
                    name: name.into(),
                    value: seconds.round(),
                    input: Some(SourceInput::written(seconds, "s")),
                });
            }
        }
        if let Some(efficiency) = positive(row, &["efficiency", "sleep_efficiency", "Sleep Efficiency"]) {
            aggregates.push(Aggregate {
                name: "efficiency_pct".into(),
                value: ((efficiency * 10.0).round() / 10.0).min(100.0),
                input: Some(SourceInput::written(efficiency, "%")),
            });
        }
        let start_at = normalized_utc(start.0)?;
        let data = SleepSessionData {
            source_id: self.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: period.map(|period| vec![period]),
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::SleepSession, None)),
            start_at: start_at.clone(),
            end_at: normalized_utc(end.0)?,
            input_start_timestamp: oura_receipt(start.1.unwrap_or_default()),
            input_end_timestamp: oura_receipt(end.1.unwrap_or_default()),
            source_reported_summaries: source_reported_summaries(&aggregates),
            asleep_seconds: None,
            in_bed_seconds: None,
            interruptions: None,
            stage_minutes: None,
        };
        self.sink.add(Candidate::new(
            RecordData::SleepSession(data),
            MatchKey::session(RecordType::SleepSession, &start_at),
            Section::Record,
        ))
    }
}

/// Place a value in time as 2.1.0 did.
///
/// - A value with a provider day and no bedtime claim keeps only the day.
/// - Otherwise the anchor is read as an instant. An anchor at exactly
///   `T00:00:00Z` is a day Oura did not place in time: it becomes an
///   `oura_day_unspecified` period, even when the row named another day type.
/// - Any other anchor is the stored instant, with the bedtime receipt or a
///   receipt built from the anchor, and keeps the row's day.
fn place(at: Placement<'_>) -> Result<Placed> {
    let receipt = at.timestamp.map(oura_receipt);
    let periods = at.period.map(|period| vec![period]);
    if let (Some(periods), None) = (&periods, &receipt) {
        return Ok(Placed {
            recorded_at: None,
            input_timestamp: missing_timestamp_receipt(),
            key_anchor: periods[0].provider_date.clone().unwrap_or_else(|| at.anchor.to_string()),
            periods: Some(periods.clone()),
        });
    }
    let normalized = normalized_utc(at.anchor)?;
    if is_midnight_utc(at.anchor) {
        let date = normalized[..10].to_string();
        return Ok(Placed {
            recorded_at: None,
            input_timestamp: missing_timestamp_receipt(),
            key_anchor: date.clone(),
            periods: Some(vec![ProviderReportingPeriod {
                period_type: ProviderReportingPeriodType::OuraDayUnspecified,
                provider_date: Some(date),
                ..unplaced_period()
            }]),
        });
    }
    Ok(Placed {
        recorded_at: Some(normalized.clone()),
        input_timestamp: receipt
            .unwrap_or_else(|| offset_timestamp_receipt(at.anchor, UtcOffsetMeaning::MeaningNotDocumented)),
        periods,
        key_anchor: normalized,
    })
}

/// `/T00:00:00(?:\.000)?Z$/`.
fn is_midnight_utc(text: &str) -> bool {
    text.ends_with("T00:00:00Z") || text.ends_with("T00:00:00.000Z")
}

fn unplaced_period() -> ProviderReportingPeriod {
    ProviderReportingPeriod {
        period_type: ProviderReportingPeriodType::OuraDayUnspecified,
        provider_period_id: None,
        provider_date: None,
        start_at: None,
        end_at: None,
        input_start_timestamp: None,
        input_end_timestamp: None,
        timezone_offset_text: None,
        timezone_offset_meaning: None,
        timezone_offset_missing_reason: Some(MissingReason::NotProvidedBySource),
    }
}

/// The day Oura assigned a value, as a provider reporting period.
fn period(day: Option<&str>, period_type: ProviderReportingPeriodType) -> Option<ProviderReportingPeriod> {
    Some(ProviderReportingPeriod { period_type, provider_date: Some(day?.to_string()), ..unplaced_period() })
}

/// The receipt for a bedtime as Oura wrote it. The offset text is kept with
/// the case Oura used, so a trailing `z` stays lowercase.
fn oura_receipt(timestamp: &str) -> InputTimestampReceipt {
    let offset = offset_suffix(timestamp);
    InputTimestampReceipt {
        timestamp_text: Some(timestamp.to_string()),
        utc_offset_meaning: offset.as_ref().map(|_| UtcOffsetMeaning::MeaningNotDocumented),
        utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
        utc_offset_text: offset,
        source_time_zone: None,
        source_time_zone_meaning: None,
        timestamp_basis: Some(InputTimestampBasis::SourceOffsetTimestamp),
        timestamp_missing_reason: None,
    }
}

/// A trailing `Z`, `z`, or `±??:??`. The digits are not checked.
fn offset_suffix(timestamp: &str) -> Option<String> {
    let trimmed = timestamp.trim();
    if trimmed.ends_with('Z') || trimmed.ends_with('z') {
        return Some(trimmed[trimmed.len() - 1..].to_string());
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 6 {
        let start = bytes.len() - 6;
        if matches!(bytes[start], b'+' | b'-') && bytes[start + 3] == b':' {
            return trimmed.get(start..).map(str::to_string);
        }
    }
    None
}

/// The number in a cell: the `average`, else the `score`, of a JSON object,
/// or the cell read as a number.
fn json_number(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if !trimmed.starts_with('{') {
        return trimmed.replace(',', "").parse::<f64>().ok().filter(|value| value.is_finite());
    }
    let Ok(Value::Object(parsed)) = json::parse(trimmed) else {
        return None;
    };
    parsed.get("average").and_then(Value::as_f64).or_else(|| parsed.get("score").and_then(Value::as_f64))
}

/// The keys of a `contributors` cell: empty for a blank cell or `{}`, and
/// nothing for a cell that is not a JSON object.
fn contributor_keys(text: Option<&str>) -> Option<Vec<String>> {
    let text = text?.trim();
    if text.is_empty() || text == "{}" {
        return Some(Vec::new());
    }
    if !text.starts_with('{') {
        return None;
    }
    match json::parse(text) {
        Ok(Value::Object(parsed)) => Some(parsed.keys().cloned().collect()),
        _ => None,
    }
}

/// The score a member-portal table holds, from the first row whose
/// contributors name any known key.
fn detect_portal(rows: &[Row]) -> Option<&'static str> {
    for row in rows {
        let Some(keys) = contributor_keys(cell(row, &["contributors"]).as_deref()) else {
            continue;
        };
        if keys.is_empty() {
            continue;
        }
        if let Some(metric) = portal_metric(&keys) {
            return Some(metric);
        }
    }
    None
}

fn portal_metric(keys: &[String]) -> Option<&'static str> {
    let count = |known: &[&str]| keys.iter().filter(|key| known.contains(&key.as_str())).count();
    let readiness = count(&CONTRIBUTORS);
    let sleep = count(&SLEEP_CONTRIBUTORS);
    let activity = count(&ACTIVITY_CONTRIBUTORS);
    let best = readiness.max(sleep).max(activity);
    if best == 0 {
        return None;
    }
    if sleep == best && sleep > readiness && sleep >= activity {
        return Some("sleep_score");
    }
    if activity == best && activity > readiness && activity >= sleep {
        return Some("activity_score");
    }
    Some("readiness")
}

/// Milliseconds for an RFC 3339 time, or 0 when it cannot be read.
fn parse_ms(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value).map(|time| time.timestamp_millis()).unwrap_or(0)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_tables_are_named_by_their_contributor_keys() {
        let keys = |list: &[&str]| list.iter().map(|key| key.to_string()).collect::<Vec<_>>();
        assert_eq!(portal_metric(&keys(&["timing", "latency"])), Some("sleep_score"));
        assert_eq!(portal_metric(&keys(&["stay_active", "recovery_time"])), Some("activity_score"));
        assert_eq!(portal_metric(&keys(&["hrv_balance"])), Some("readiness"));
        // A tie with readiness keys stays readiness.
        assert_eq!(portal_metric(&keys(&["hrv_balance", "timing"])), Some("readiness"));
        assert_eq!(portal_metric(&keys(&["unknown"])), None);
    }

    #[test]
    fn a_bedtime_receipt_keeps_the_offset_as_written() {
        assert_eq!(offset_suffix("2026-08-09T06:08:00-07:00").as_deref(), Some("-07:00"));
        assert_eq!(offset_suffix("2026-08-09T06:08:00z").as_deref(), Some("z"));
        assert_eq!(offset_suffix("2026-08-09T06:08:00"), None);
        assert_eq!(
            oura_receipt("2026-08-09 06:08").utc_offset_missing_reason,
            Some(MissingReason::NotProvidedBySource)
        );
    }

    #[test]
    fn a_json_spo2_cell_reads_its_average_then_its_score() {
        assert_eq!(json_number("{\"average\": 97.2}"), Some(97.2));
        assert_eq!(json_number("{\"score\": 3}"), Some(3.0));
        assert_eq!(json_number("{\"other\": 3}"), None);
        assert_eq!(json_number("1,234"), Some(1234.0));
    }

    #[test]
    fn contributor_keys_tell_blank_from_unreadable() {
        assert_eq!(contributor_keys(Some("{}")), Some(Vec::new()));
        assert_eq!(contributor_keys(Some(" ")), Some(Vec::new()));
        assert_eq!(contributor_keys(Some("12")), None);
        assert_eq!(contributor_keys(Some("{bad")), None);
        assert_eq!(contributor_keys(Some("{\"b\":1,\"a\":2}")), Some(vec!["b".to_string(), "a".to_string()]));
    }
}
