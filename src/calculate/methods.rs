//! The one-record calculation methods added in 2.1.2 (D230–D242).
//!
//! Each method reads the target record and, where its definition says so,
//! other current records from the same source, the workout a report links to,
//! the athlete profile, or heart-rate samples. A missing input is named in
//! `missing` and the result is null; nothing is guessed. Nothing is written.
//!
//! The 2.1.1 operations stay in `record.rs` with their old arithmetic, so an
//! edited calculation file still gives the answer it gave before (D230).

use std::collections::BTreeMap;

use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Serialize;

use super::definition::{RecoverySettings, RollingMeanSettings, TrimpSettings, WindowChangeSettings};
use super::record::{parse_millis, Context, FieldRead, OneRecordResult, StoredRecord};
use crate::json::{self, Map, Value};
use crate::model::{time, EvidenceReference};

const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;
/// One pound in kilograms, exactly.
pub const POUND_IN_KG: f64 = 0.45359237;

/// The athlete profile fields some methods read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub time_zone: Option<String>,
    pub sex: Option<String>,
}

/// One heart-rate sample from a series: the instant in milliseconds, the
/// rate, and the record it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesSample {
    pub at_ms: i64,
    pub bpm: f64,
    pub reference: EvidenceReference,
}

fn round_to(value: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (value * factor).round() / factor
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// The sample standard deviation (n − 1). None for fewer than two values.
fn sample_standard_deviation(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let center = mean(values);
    let squares = values.iter().map(|value| (value - center).powi(2)).sum::<f64>();
    Some((squares / (values.len() - 1) as f64).sqrt())
}

/// The athlete's time zone, or UTC, with the name to report.
fn time_zone(profile: &Profile) -> (Tz, String) {
    match profile.time_zone.as_deref().and_then(|name| name.parse::<Tz>().ok().map(|zone| (zone, name))) {
        Some((zone, name)) => (zone, name.to_string()),
        None => (Tz::UTC, "UTC".to_string()),
    }
}

fn local_date(at_ms: i64, zone: Tz) -> Option<NaiveDate> {
    Utc.timestamp_millis_opt(at_ms).single().map(|instant| instant.with_timezone(&zone).date_naive())
}

fn reference(record: &StoredRecord) -> EvidenceReference {
    EvidenceReference { record_id: record.record_id.clone(), revision: record.revision }
}

fn standard_value(data: &Value) -> Option<f64> {
    data.get("value").and_then(|value| value.get("standard_value")).and_then(Value::as_f64)
}

/// `field_basis.value`, or `unstated` for a record without a basis (D233).
fn basis_of(data: &Value) -> String {
    data.get("field_basis")
        .and_then(|basis| basis.get("value"))
        .and_then(Value::as_str)
        .unwrap_or("unstated")
        .to_string()
}

fn instant(data: &Value, field: &str) -> Option<i64> {
    data.get(field).and_then(Value::as_str).and_then(parse_millis)
}

/// Heart-rate samples `(offset_ms, bpm)`, sorted by offset.
fn sorted_samples(value: &Value) -> Vec<(i64, f64)> {
    let mut samples: Vec<(i64, f64)> = value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let bpm = item.get("bpm")?.as_f64()?;
                    let offset = item.get("offset_ms").and_then(Value::as_i64).unwrap_or(0);
                    Some((offset, bpm))
                })
                .collect()
        })
        .unwrap_or_default();
    samples.sort_by_key(|sample| sample.0);
    samples
}

/// How long the workout lasted, from `start_at` and `end_at`.
fn workout_span_ms(data: &Value) -> Option<i64> {
    let start = instant(data, "start_at")?;
    let end = instant(data, "end_at")?;
    (end >= start).then_some(end - start)
}

/// Each sample lasts until the next sample, or until the workout end for the
/// last one, but never longer than `max_gap_ms`. Returns each sample's covered
/// milliseconds and the uncovered milliseconds, including any time before the
/// first sample (D240).
fn covered_durations(samples: &[(i64, f64)], end_ms: Option<i64>, max_gap_ms: i64) -> (Vec<(f64, i64)>, i64) {
    let mut out = Vec::with_capacity(samples.len());
    let mut uncovered = samples.first().map_or(0, |first| first.0.max(0));
    for (index, (offset, bpm)) in samples.iter().enumerate() {
        let next = match samples.get(index + 1) {
            Some(next) => next.0,
            None => end_ms.filter(|end| end > offset).unwrap_or(*offset),
        };
        let gap = (next - offset).max(0);
        let covered = gap.min(max_gap_ms.max(0));
        uncovered += gap - covered;
        out.push((*bpm, covered));
    }
    (out, uncovered)
}

/// The heart rate at `at`: the sample there, else straight-line interpolation
/// between neighbours no more than `max_gap_ms` apart, else the nearest sample
/// within `tolerance_ms`.
fn heart_rate_at(samples: &[(i64, f64)], at: i64, max_gap_ms: i64, tolerance_ms: i64) -> Option<f64> {
    if let Some((_, bpm)) = samples.iter().find(|(offset, _)| *offset == at) {
        return Some(*bpm);
    }
    for pair in samples.windows(2) {
        let ((left_at, left), (right_at, right)) = (pair[0], pair[1]);
        if left_at < at && at < right_at && right_at - left_at <= max_gap_ms {
            let share = (at - left_at) as f64 / (right_at - left_at) as f64;
            return Some(left + (right - left) * share);
        }
    }
    samples
        .iter()
        .filter(|(offset, _)| (offset - at).abs() <= tolerance_ms)
        .min_by_key(|(offset, _)| (offset - at).abs())
        .map(|(_, bpm)| *bpm)
}

// ---- Results ---------------------------------------------------------------

/// The earlier values a change compares against.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    pub mean: f64,
    /// The sample standard deviation (n − 1) of the compared values.
    pub standard_deviation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nights: Option<usize>,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowChangeResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basis: Option<String>,
    /// The target value: the mean of the target's day so far, or the night.
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_day_readings: Option<usize>,
    pub change: f64,
    pub change_percent: Option<f64>,
    /// The change in standard deviations of the compared values.
    pub standardized_change: Option<f64>,
    pub comparison: Comparison,
    pub compared_records: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoveryResult {
    pub drop_bpm: f64,
    pub end_bpm: f64,
    pub later_bpm: f64,
    pub end_offset_ms: i64,
    pub later_offset_ms: i64,
    /// `recovery segment` or `workout end`.
    pub anchor: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegularityResult {
    /// −100 (opposite states) to 100 (the same state 24 hours apart).
    pub index: f64,
    pub day_pairs: usize,
    pub days_observed: usize,
    pub window_days: u32,
    pub window_from: String,
    pub window_to: String,
    pub sessions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MovementVolume {
    pub movement: Option<String>,
    pub volume: f64,
    pub sets: usize,
    pub reps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VolumeResult {
    pub volume: f64,
    pub weight_unit: &'static str,
    pub sets: usize,
    pub reps: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_movement: Option<Vec<MovementVolume>>,
    pub unstated_load_basis_sets: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffortLoadResult {
    pub load: f64,
    pub unit: &'static str,
    pub rating: f64,
    pub rating_scale: String,
    /// `10-point` or `6-20`. Loads on different scales are never comparable.
    pub scale_family: &'static str,
    pub duration_minutes: f64,
    /// `report` or `linked workout session`.
    pub duration_from: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrimpResult {
    pub load: f64,
    pub weighting: String,
    pub coefficient: f64,
    pub exponent: f64,
    pub covered_minutes: f64,
    pub uncovered_minutes: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZonesResult {
    pub seconds: Vec<f64>,
    pub uncovered_seconds: f64,
    pub boundaries_bpm: Vec<f64>,
    pub maximum_heart_rate_bpm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestingResult {
    pub bpm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_start_offset_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_start_at: Option<String>,
    pub window_minutes: f64,
    pub samples_in_window: usize,
    pub samples_read: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EfficiencyResult {
    pub efficiency: f64,
    pub percent: f64,
    pub asleep_seconds: f64,
    pub in_bed_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StageSplitResult {
    pub stages: Map<String, Value>,
    pub asleep_minutes: f64,
    pub awake_minutes: f64,
    pub total_minutes: f64,
    /// Each sleep stage as a percentage of asleep minutes.
    pub percent_of_asleep: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClockTimesResult {
    pub start_at: String,
    pub midpoint_at: String,
    pub end_at: String,
    pub local_start: String,
    pub local_midpoint: String,
    pub local_end: String,
    pub time_zone: String,
    /// `athlete_profile.default_time_zone`, `source offset`, or `utc`.
    pub time_zone_basis: &'static str,
    pub duration_hours: f64,
}

// ---- earlier_window_change ---------------------------------------------------

pub(super) fn window_change(
    context: &mut Context<'_>,
    settings: &WindowChangeSettings,
    record: &StoredRecord,
    related: &[StoredRecord],
    profile: &Profile,
) -> Option<OneRecordResult> {
    match record.record_type.as_str() {
        "point_measurement" | "measurement_sample" => measurement_change(context, settings, record, related, profile),
        "sleep_session" => sleep_change(context, settings, record, related),
        other => {
            context
                .missing
                .push(format!("a point measurement, measurement sample, or sleep session; this record is a {other}"));
            None
        }
    }
}

fn measurement_change(
    context: &mut Context<'_>,
    settings: &WindowChangeSettings,
    record: &StoredRecord,
    related: &[StoredRecord],
    profile: &Profile,
) -> Option<OneRecordResult> {
    let data = &record.data;
    let quantity = data.get("quantity").and_then(Value::as_str).unwrap_or("").to_string();
    context.fields_read.push(FieldRead {
        field: "quantity".into(),
        present: !quantity.is_empty(),
        value: Some(quantity.clone()),
    });
    let value = standard_value(data);
    context.fields_read.push(FieldRead { field: "value.standard_value".into(), present: value.is_some(), value: None });
    let at = instant(data, "recorded_at");
    context.fields_read.push(FieldRead { field: "recorded_at".into(), present: at.is_some(), value: None });
    let Some(value) = value else {
        context.missing.push("value.standard_value".into());
        return None;
    };
    let Some(at) = at else {
        context.missing.push("recorded_at; a reading with only a provider period has no day to compare".into());
        return None;
    };
    let basis = basis_of(data);
    let (zone, zone_name) = time_zone(profile);
    let day = local_date(at, zone)?;
    let window_from = day - chrono::Duration::days(i64::from(settings.window_days));
    let mut same_day = vec![value];
    let mut daily: BTreeMap<NaiveDate, Vec<f64>> = BTreeMap::new();
    let (mut other_basis, mut later) = (0usize, 0usize);
    for other in related.iter().filter(|other| other.record_id != record.record_id) {
        let (Some(other_value), Some(other_at)) = (standard_value(&other.data), instant(&other.data, "recorded_at"))
        else {
            continue;
        };
        let Some(other_day) = local_date(other_at, zone) else {
            continue;
        };
        if other_day > day || (other_day == day && other_at > at) {
            later += 1;
            continue;
        }
        if other_day < window_from {
            continue;
        }
        if basis_of(&other.data) != basis {
            other_basis += 1;
            continue;
        }
        context.inputs.push(reference(other));
        if other_day == day {
            same_day.push(other_value);
        } else {
            daily.entry(other_day).or_default().push(other_value);
        }
    }
    context.exclusions.push("other sources, record types, and quantities are not compared".into());
    context.exclusions.push("readings after this one are excluded".into());
    if later > 0 {
        context.exclusions.push(format!("{later} later readings excluded"));
    }
    if other_basis > 0 {
        context.exclusions.push(format!("{other_basis} readings with a basis other than {basis} excluded"));
    }
    let days = daily.len();
    if days < settings.minimum_days as usize {
        context.missing.push(format!(
            "found {days} earlier days of {quantity} in the {} days before {day}, need {}",
            settings.window_days, settings.minimum_days
        ));
        return None;
    }
    let daily_means: Vec<f64> = daily.values().map(|values| mean(values)).collect();
    let compared_records = daily.values().map(Vec::len).sum::<usize>();
    let target = mean(&same_day);
    let center = mean(&daily_means);
    let spread = sample_standard_deviation(&daily_means);
    let change = target - center;
    Some(OneRecordResult::WindowChange(WindowChangeResult {
        quantity: Some(quantity),
        field: None,
        basis: Some(basis),
        value: round_to(target, 3),
        target_day_readings: Some(same_day.len()),
        change: round_to(change, 3),
        change_percent: (center != 0.0).then(|| round_to(change / center * 100.0, 1)),
        standardized_change: spread.filter(|spread| *spread > 0.0).map(|spread| round_to(change / spread, 2)),
        comparison: Comparison {
            mean: round_to(center, 3),
            standard_deviation: spread.map(|spread| round_to(spread, 3)),
            days: Some(days),
            nights: None,
            from: window_from.to_string(),
            to: (day - chrono::Duration::days(1)).to_string(),
        },
        compared_records,
        time_zone: Some(zone_name),
    }))
}

fn sleep_change(
    context: &mut Context<'_>,
    settings: &WindowChangeSettings,
    record: &StoredRecord,
    related: &[StoredRecord],
) -> Option<OneRecordResult> {
    let field = settings.field.clone().unwrap_or_else(|| "asleep_seconds".to_string());
    let value = context.lookup(&field).and_then(Value::as_f64);
    let (start, end) = (instant(&record.data, "start_at"), instant(&record.data, "end_at"));
    let Some(value) = value else {
        context.missing.push(field);
        return None;
    };
    let (Some(start), Some(end)) = (start, end) else {
        context.missing.push("start_at and end_at".into());
        return None;
    };
    let main_ms = (settings.main_sleep_hours * 3_600_000.0) as i64;
    if end - start < main_ms {
        context.missing.push(format!(
            "a main sleep of at least {} h; this session spans {:.1} h",
            settings.main_sleep_hours,
            (end - start) as f64 / 3_600_000.0
        ));
        return None;
    }
    let window_from = start - i64::from(settings.window_days) * DAY_MS;
    let mut values = Vec::new();
    let (mut naps, mut later, mut without_field) = (0usize, 0usize, 0usize);
    for other in related.iter().filter(|other| other.record_id != record.record_id) {
        let (Some(other_start), Some(other_end)) = (instant(&other.data, "start_at"), instant(&other.data, "end_at"))
        else {
            continue;
        };
        if other_end > start {
            later += 1;
            continue;
        }
        if other_end < window_from {
            continue;
        }
        if other_end - other_start < main_ms {
            naps += 1;
            continue;
        }
        match other.data.get(&field).and_then(Value::as_f64) {
            Some(other_value) => {
                values.push(other_value);
                context.inputs.push(reference(other));
            }
            None => without_field += 1,
        }
    }
    context.exclusions.push("other sources are not compared".into());
    context.exclusions.push("sessions that end after this one starts are excluded".into());
    if later > 0 {
        context.exclusions.push(format!("{later} sessions that overlap this one excluded"));
    }
    if naps > 0 {
        context.exclusions.push(format!("{naps} naps shorter than {} h excluded", settings.main_sleep_hours));
    }
    if without_field > 0 {
        context.exclusions.push(format!("{without_field} earlier main sleeps without {field} excluded"));
    }
    if values.len() < settings.minimum_days as usize {
        context.missing.push(format!(
            "found {} earlier main sleeps in the {} days before this one, need {}",
            values.len(),
            settings.window_days,
            settings.minimum_days
        ));
        return None;
    }
    let center = mean(&values);
    let spread = sample_standard_deviation(&values);
    let change = value - center;
    Some(OneRecordResult::WindowChange(WindowChangeResult {
        quantity: None,
        field: Some(field),
        basis: None,
        value: round_to(value, 3),
        target_day_readings: None,
        change: round_to(change, 3),
        change_percent: (center != 0.0).then(|| round_to(change / center * 100.0, 1)),
        standardized_change: spread.filter(|spread| *spread > 0.0).map(|spread| round_to(change / spread, 2)),
        comparison: Comparison {
            mean: round_to(center, 3),
            standard_deviation: spread.map(|spread| round_to(spread, 3)),
            days: None,
            nights: Some(values.len()),
            from: time::format_utc_millis(window_from),
            to: time::format_utc_millis(start),
        },
        compared_records: values.len(),
        time_zone: None,
    }))
}

// ---- drop_after_effort_end -----------------------------------------------------

pub(super) fn drop_after_effort_end(context: &mut Context<'_>, settings: &RecoverySettings) -> Option<OneRecordResult> {
    let samples = context.lookup(&settings.sample_field).map(sorted_samples).unwrap_or_default();
    if samples.is_empty() {
        context.missing.push(settings.sample_field.clone());
        return None;
    }
    let recovery_start = context
        .data
        .get("segments")
        .and_then(Value::as_array)
        .and_then(|segments| {
            segments.iter().find(|segment| {
                segment
                    .get("label")
                    .and_then(Value::as_str)
                    .is_some_and(|label| label.eq_ignore_ascii_case(&settings.recovery_label))
            })
        })
        .and_then(|segment| segment.get("start_offset_ms"))
        .and_then(Value::as_i64);
    let (anchor, end_offset) = match recovery_start {
        Some(offset) => ("recovery segment", offset),
        None => match workout_span_ms(context.data) {
            Some(span) => ("workout end", span),
            None => {
                context.missing.push("start_at and end_at, or a recovery segment with start_offset_ms".into());
                return None;
            }
        },
    };
    let Some(end_bpm) = heart_rate_at(&samples, end_offset, settings.max_interpolation_gap_ms, settings.tolerance_ms)
    else {
        context.missing.push(format!(
            "a heart-rate sample within {} ms of the end of effort at offset {end_offset}",
            settings.tolerance_ms
        ));
        return None;
    };
    let later_offset = end_offset + settings.later_offset_ms;
    let Some(later_bpm) =
        heart_rate_at(&samples, later_offset, settings.max_interpolation_gap_ms, settings.tolerance_ms)
    else {
        context.missing.push(format!(
            "heart-rate samples {} s after the end of effort, at offset {later_offset}; many devices stop recording when the workout ends",
            settings.later_offset_ms / 1000
        ));
        return None;
    };
    Some(OneRecordResult::Recovery(RecoveryResult {
        drop_bpm: round_to(end_bpm - later_bpm, 1),
        end_bpm: round_to(end_bpm, 1),
        later_bpm: round_to(later_bpm, 1),
        end_offset_ms: end_offset,
        later_offset_ms: later_offset,
        anchor,
    }))
}

// ---- sleep_regularity_index ---------------------------------------------------------

pub(super) fn sleep_regularity_index(
    context: &mut Context<'_>,
    window_days: u32,
    minimum_day_pairs: u32,
    record: &StoredRecord,
    related: &[StoredRecord],
) -> Option<OneRecordResult> {
    let end = instant(&record.data, "end_at");
    context.fields_read.push(FieldRead { field: "start_at".into(), present: true, value: None });
    context.fields_read.push(FieldRead { field: "end_at".into(), present: end.is_some(), value: None });
    let Some(window_to) = end else {
        context.missing.push("end_at".into());
        return None;
    };
    let window_days = window_days.max(2);
    let window_from = window_to - i64::from(window_days) * DAY_MS;
    let mut periods: Vec<(i64, i64)> = Vec::new();
    let mut sessions = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for session in related.iter().chain(std::iter::once(record)) {
        if !seen.insert(session.record_id.as_str()) {
            continue;
        }
        let (Some(start), Some(end)) = (instant(&session.data, "start_at"), instant(&session.data, "end_at")) else {
            continue;
        };
        if end <= window_from || start >= window_to {
            continue;
        }
        sessions += 1;
        if session.record_id != record.record_id {
            context.inputs.push(reference(session));
        }
        periods.push((start, end));
    }
    periods.sort_unstable();
    let asleep = |at: i64| periods.iter().any(|(start, end)| *start <= at && at < *end);
    let observed: Vec<bool> = (0..i64::from(window_days))
        .map(|day| {
            let (from, to) = (window_from + day * DAY_MS, window_from + (day + 1) * DAY_MS);
            periods.iter().any(|(start, end)| *start < to && *end > from)
        })
        .collect();
    let days_observed = observed.iter().filter(|seen| **seen).count();
    let mut day_pairs = 0usize;
    let mut agreements = 0u64;
    for day in 0..observed.len() - 1 {
        if !(observed[day] && observed[day + 1]) {
            continue;
        }
        day_pairs += 1;
        let day_start = window_from + day as i64 * DAY_MS;
        for minute in 0..1440 {
            let at = day_start + minute * MINUTE_MS;
            if asleep(at) == asleep(at + DAY_MS) {
                agreements += 1;
            }
        }
    }
    context.exclusions.push("awake time inside a sleep session counts as asleep".into());
    context.exclusions.push("other sources are not compared".into());
    if days_observed < observed.len() {
        context.exclusions.push(format!(
            "{} days without a sleep session are unobserved; pairs that touch them are left out",
            observed.len() - days_observed
        ));
    }
    if day_pairs < minimum_day_pairs as usize {
        context.missing.push(format!(
            "found {day_pairs} pairs of consecutive observed days in the {window_days} days before this session ended, need {minimum_day_pairs}"
        ));
        return None;
    }
    let share = agreements as f64 / (day_pairs as f64 * 1440.0);
    Some(OneRecordResult::SleepRegularity(RegularityResult {
        index: round_to(-100.0 + 200.0 * share, 1),
        day_pairs,
        days_observed,
        window_days,
        window_from: time::format_utc_millis(window_from),
        window_to: time::format_utc_millis(window_to),
        sessions,
    }))
}

// ---- volume_load ------------------------------------------------------------------------

/// Kilograms per unit for the weight units `volume_load` recognises.
pub fn kilograms_per(unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "kg" | "kgs" | "kilogram" | "kilograms" => Some(1.0),
        "lb" | "lbs" | "pound" | "pounds" => Some(POUND_IN_KG),
        _ => None,
    }
}

pub(super) fn volume_load(context: &mut Context<'_>, field: &str) -> Option<OneRecordResult> {
    let Some(sets) = context.lookup(field).and_then(Value::as_array) else {
        context.missing.push(field.to_string());
        if context.data.get("text_result").is_some() {
            context.missing.push("text_result is text, so volume cannot be made".into());
        }
        return None;
    };
    if sets.is_empty() {
        context.missing.push(field.to_string());
        return None;
    }
    let mut total = 0.0;
    let mut reps_total = 0.0;
    let mut unstated = 0usize;
    let mut movements: Vec<MovementVolume> = Vec::new();
    for (index, set) in sets.iter().enumerate() {
        let reps = set.get("reps").and_then(Value::as_f64);
        let weight = set.get("weight").and_then(Value::as_f64);
        let unit = set.get("weight_unit").and_then(Value::as_str);
        let (Some(reps), Some(weight), Some(unit)) = (reps, weight, unit) else {
            context.missing.push(format!("set {} reps, weight, and weight_unit", index + 1));
            return None;
        };
        let Some(per_unit) = kilograms_per(unit) else {
            context.missing.push(format!("set {} weight_unit '{unit}' is not kg or lb", index + 1));
            return None;
        };
        let implements = match set.get("load_basis").and_then(Value::as_str) {
            Some("per_implement") => set.get("implement_count").and_then(Value::as_f64).unwrap_or(1.0),
            Some(_) => 1.0,
            None => {
                unstated += 1;
                1.0
            }
        };
        let volume = reps * weight * per_unit * implements;
        total += volume;
        reps_total += reps;
        let movement = set.get("movement").and_then(Value::as_str).map(|name| name.trim().to_string());
        match movements.iter_mut().find(|entry| entry.movement == movement) {
            Some(entry) => {
                entry.volume += volume;
                entry.sets += 1;
                entry.reps += reps;
            }
            None => movements.push(MovementVolume { movement, volume, sets: 1, reps }),
        }
    }
    let named = movements.iter().any(|entry| entry.movement.is_some());
    for entry in &mut movements {
        entry.volume = round_to(entry.volume, 3);
    }
    if unstated > 0 {
        context.exclusions.push(format!("{unstated} sets state no load basis and are counted as written"));
    }
    Some(OneRecordResult::VolumeLoad(VolumeResult {
        volume: round_to(total, 3),
        weight_unit: "kg",
        sets: sets.len(),
        reps: reps_total,
        by_movement: named.then_some(movements),
        unstated_load_basis_sets: unstated,
    }))
}

// ---- session_effort_load -------------------------------------------------------------------

/// The scale family and bounds of a session-effort rating scale.
pub fn effort_scale(scale: &str) -> Option<(&'static str, f64, f64)> {
    match scale.trim().to_ascii_lowercase().replace(' ', "").as_str() {
        "0-10" | "cr10" | "cr-10" => Some(("10-point", 0.0, 10.0)),
        "1-10" => Some(("10-point", 1.0, 10.0)),
        "6-20" => Some(("6-20", 6.0, 20.0)),
        _ => None,
    }
}

pub(super) fn session_effort_load(
    context: &mut Context<'_>,
    duration_field: &str,
    effort_field: &str,
    record: &StoredRecord,
    linked_workout: Option<&StoredRecord>,
) -> Option<OneRecordResult> {
    let report_type = record.data.get("report_type").and_then(Value::as_str).unwrap_or("").to_string();
    context.fields_read.push(FieldRead {
        field: "report_type".into(),
        present: !report_type.is_empty(),
        value: Some(report_type.clone()),
    });
    if record.record_type != "athlete_report" || report_type != "session_effort" {
        let what = if record.record_type == "athlete_report" {
            format!("a {report_type} athlete report")
        } else {
            format!("a {}", record.record_type)
        };
        context.missing.push(format!(
            "a session_effort athlete report; this is {what}, and only a session effort becomes training load"
        ));
        return None;
    }
    let rating = context.lookup(effort_field).and_then(Value::as_f64);
    let scale = context.lookup("rating_scale").and_then(Value::as_str).map(str::to_string);
    let (Some(rating), Some(scale)) = (rating, scale) else {
        context.missing.push(format!("{effort_field} and rating_scale"));
        return None;
    };
    let Some((family, low, high)) = effort_scale(&scale) else {
        context.missing.push(format!("a rating_scale of 0-10, 1-10, or 6-20; found '{scale}'"));
        return None;
    };
    if rating < low || rating > high {
        context.missing.push(format!("a rating on the {scale} scale; found {rating}"));
        return None;
    }
    let (minutes, from) = match context.lookup(duration_field).and_then(Value::as_f64) {
        Some(seconds) => (seconds / 60.0, "report"),
        None => {
            let linked = context.lookup("workout_session_id").is_some();
            match linked_workout.and_then(|workout| workout_span_ms(&workout.data)) {
                Some(span) if linked => {
                    if let Some(workout) = linked_workout {
                        context.inputs.push(reference(workout));
                    }
                    (span as f64 / 60_000.0, "linked workout session")
                }
                _ => {
                    context.missing.push(format!(
                        "{duration_field}, or a workout_session_id naming a current workout with a start and end"
                    ));
                    return None;
                }
            }
        }
    };
    Some(OneRecordResult::EffortLoad(EffortLoadResult {
        load: round_to(rating * minutes, 1),
        unit: "AU",
        rating,
        rating_scale: scale,
        scale_family: family,
        duration_minutes: round_to(minutes, 1),
        duration_from: from,
    }))
}

// ---- banister_trimp and time_in_zones -------------------------------------------------------

pub(super) fn banister_trimp(
    context: &mut Context<'_>,
    settings: &TrimpSettings,
    profile: &Profile,
) -> Option<OneRecordResult> {
    let samples_value = context.lookup(&settings.sample_field).cloned();
    let rest = context.lookup(&settings.rest_field).and_then(Value::as_f64);
    let maximum = context.lookup(&settings.maximum_field).and_then(Value::as_f64);
    context.fields_read.push(FieldRead {
        field: "athlete_profile.sex".into(),
        present: profile.sex.is_some(),
        value: profile.sex.clone(),
    });
    if samples_value.is_none() {
        context.missing.push(settings.sample_field.clone());
    }
    if rest.is_none() {
        context.missing.push(settings.rest_field.clone());
    }
    if maximum.is_none() {
        context.missing.push(settings.maximum_field.clone());
    }
    let weighting = match profile.sex.as_deref() {
        Some("male") => Some(("male", settings.male)),
        Some("female") => Some(("female", settings.female)),
        _ => None,
    };
    if weighting.is_none() {
        context.missing.push("athlete_profile.sex, which chooses Banister's weighting; it is not guessed".into());
    }
    let (Some(samples_value), Some(rest), Some(maximum), Some((label, (coefficient, exponent)))) =
        (samples_value, rest, maximum, weighting)
    else {
        return None;
    };
    if maximum <= rest {
        context.missing.push(format!("{} is not above {}", settings.maximum_field, settings.rest_field));
        return None;
    }
    let samples = sorted_samples(&samples_value);
    let (durations, uncovered) = covered_durations(&samples, workout_span_ms(context.data), settings.max_gap_ms);
    let mut load = 0.0;
    let mut covered = 0i64;
    for (bpm, duration) in durations {
        let reserve = ((bpm - rest) / (maximum - rest)).clamp(0.0, 1.0);
        load += duration as f64 / 60_000.0 * reserve * coefficient * (exponent * reserve).exp();
        covered += duration;
    }
    if uncovered > 0 {
        context.exclusions.push(format!(
            "{:.1} minutes without a sample within {} s are not counted",
            uncovered as f64 / 60_000.0,
            settings.max_gap_ms / 1000
        ));
    }
    Some(OneRecordResult::Trimp(TrimpResult {
        load: round_to(load, 1),
        weighting: label.to_string(),
        coefficient,
        exponent,
        covered_minutes: round_to(covered as f64 / 60_000.0, 1),
        uncovered_minutes: round_to(uncovered as f64 / 60_000.0, 1),
    }))
}

pub(super) fn time_in_zones(
    context: &mut Context<'_>,
    sample_field: &str,
    maximum_field: &str,
    cuts: &[f64],
    max_gap_ms: i64,
) -> Option<OneRecordResult> {
    let samples_value = context.lookup(sample_field).cloned();
    let maximum = context.lookup(maximum_field).and_then(Value::as_f64);
    if samples_value.is_none() {
        context.missing.push(sample_field.to_string());
    }
    let Some(maximum) = maximum.filter(|maximum| *maximum > 0.0) else {
        context.missing.push(maximum_field.to_string());
        return None;
    };
    let samples = sorted_samples(samples_value.as_ref().unwrap_or(&Value::Null));
    if samples.is_empty() {
        return None;
    }
    let (durations, uncovered) = covered_durations(&samples, workout_span_ms(context.data), max_gap_ms);
    let mut seconds = vec![0.0; cuts.len() + 1];
    for (bpm, duration) in durations {
        let fraction = bpm / maximum;
        let zone = cuts.iter().position(|cut| fraction < *cut).unwrap_or(cuts.len());
        seconds[zone] += duration as f64 / 1000.0;
    }
    if uncovered > 0 {
        context.exclusions.push(format!(
            "{:.0} seconds without a sample within {} s are not in any zone",
            uncovered as f64 / 1000.0,
            max_gap_ms / 1000
        ));
    }
    Some(OneRecordResult::TimeInZones(ZonesResult {
        seconds: seconds.into_iter().map(|value| round_to(value, 3)).collect(),
        uncovered_seconds: round_to(uncovered as f64 / 1000.0, 3),
        boundaries_bpm: cuts.iter().map(|cut| round_to(cut * maximum, 1)).collect(),
        maximum_heart_rate_bpm: maximum,
    }))
}

// ---- lowest_rolling_mean ----------------------------------------------------------------------

/// The lowest mean over `[t_i, t_i + window]` among windows with enough
/// samples, no long gap, and a last sample near the window's end. Returns the
/// mean, the start, and the sample count.
fn lowest_window(times: &[(i64, f64)], settings: &RollingMeanSettings) -> Option<(f64, i64, usize)> {
    let mut best: Option<(f64, i64, usize)> = None;
    let mut end = 0;
    for start in 0..times.len() {
        let window_end = times[start].0 + settings.window_ms;
        end = end.max(start);
        while end + 1 < times.len() && times[end + 1].0 <= window_end {
            end += 1;
        }
        let window = &times[start..=end];
        let count = window.len();
        let gaps_ok = window.windows(2).all(|pair| pair[1].0 - pair[0].0 <= settings.max_gap_ms);
        let reaches_end = window_end - window[count - 1].0 <= settings.max_gap_ms;
        if count < settings.minimum_samples || !gaps_ok || !reaches_end {
            continue;
        }
        let window_mean = window.iter().map(|(_, bpm)| bpm).sum::<f64>() / count as f64;
        if best.is_none_or(|(lowest, _, _)| window_mean < lowest) {
            best = Some((window_mean, times[start].0, count));
        }
    }
    best
}

pub(super) fn lowest_rolling_mean(
    context: &mut Context<'_>,
    settings: &RollingMeanSettings,
    record: &StoredRecord,
    series: &[SeriesSample],
) -> Option<OneRecordResult> {
    let (times, from_series): (Vec<(i64, f64)>, bool) = match record.record_type.as_str() {
        "sleep_session" => {
            context.fields_read.push(FieldRead {
                field: "heart_rate samples from this source during the session".into(),
                present: !series.is_empty(),
                value: None,
            });
            for sample in series {
                context.inputs.push(sample.reference.clone());
            }
            (series.iter().map(|sample| (sample.at_ms, sample.bpm)).collect(), true)
        }
        _ => (context.lookup(&settings.sample_field).map(sorted_samples).unwrap_or_default(), false),
    };
    if times.is_empty() {
        context.missing.push(if from_series {
            "heart_rate samples from this source during the session".to_string()
        } else {
            settings.sample_field.clone()
        });
        return None;
    }
    let Some((bpm, start, count)) = lowest_window(&times, settings) else {
        context.missing.push(format!(
            "a {}-minute window with at least {} samples and no gap over {} s",
            settings.window_ms as f64 / 60_000.0,
            settings.minimum_samples,
            settings.max_gap_ms / 1000
        ));
        return None;
    };
    Some(OneRecordResult::Resting(RestingResult {
        bpm: round_to(bpm, 1),
        window_start_offset_ms: (!from_series).then_some(start),
        window_start_at: from_series.then(|| time::format_utc_millis(start)),
        window_minutes: settings.window_ms as f64 / 60_000.0,
        samples_in_window: count,
        samples_read: times.len(),
    }))
}

// ---- sleep_efficiency, sleep_stage_split, sleep_clock_times ----------------------------------

pub(super) fn sleep_efficiency(
    context: &mut Context<'_>,
    asleep_field: &str,
    in_bed_field: &str,
) -> Option<OneRecordResult> {
    let asleep = context.lookup(asleep_field).and_then(Value::as_f64);
    let in_bed = context.lookup(in_bed_field).and_then(Value::as_f64);
    if asleep.is_none() {
        context.missing.push(asleep_field.to_string());
    }
    if in_bed.is_none() || in_bed == Some(0.0) {
        context.missing.push(in_bed_field.to_string());
    }
    let (Some(asleep), Some(in_bed)) = (asleep, in_bed.filter(|value| *value > 0.0)) else {
        return None;
    };
    if asleep > in_bed {
        context.missing.push(format!("{asleep_field} is more than {in_bed_field}, so the record is inconsistent"));
        return None;
    }
    Some(OneRecordResult::SleepEfficiency(EfficiencyResult {
        efficiency: round_to(asleep / in_bed, 4),
        percent: round_to(asleep / in_bed * 100.0, 1),
        asleep_seconds: asleep,
        in_bed_seconds: in_bed,
    }))
}

pub(super) fn sleep_stage_split(context: &mut Context<'_>, field: &str) -> Option<OneRecordResult> {
    let Some(stages) = context.lookup(field).and_then(Value::as_object).cloned() else {
        context.missing.push(field.to_string());
        return None;
    };
    let mut named = Map::new();
    let (mut asleep, mut awake) = (0.0, 0.0);
    for (name, minutes) in &stages {
        let Some(minutes) = minutes.as_f64() else {
            continue;
        };
        named.insert(name.clone(), json::to_value(&minutes));
        if name.eq_ignore_ascii_case("awake") {
            awake += minutes;
        } else {
            asleep += minutes;
        }
    }
    let mut shares = Map::new();
    if asleep > 0.0 {
        for (name, minutes) in &named {
            if !name.eq_ignore_ascii_case("awake") {
                let minutes = minutes.as_f64().unwrap_or(0.0);
                shares.insert(name.clone(), json::to_value(&round_to(minutes / asleep * 100.0, 1)));
            }
        }
    }
    context.exclusions.push("a stage named awake is counted apart from asleep minutes".into());
    Some(OneRecordResult::StageSplit(StageSplitResult {
        stages: named,
        asleep_minutes: asleep,
        awake_minutes: awake,
        total_minutes: asleep + awake,
        percent_of_asleep: shares,
    }))
}

/// `+HH:MM`, `-HH:MM`, or `Z` as a fixed offset.
fn fixed_offset(text: &str) -> Option<FixedOffset> {
    if text == "Z" {
        return FixedOffset::east_opt(0);
    }
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (hours, minutes) = text[1..].split_once(':')?;
    FixedOffset::east_opt(sign * (hours.parse::<i32>().ok()? * 3600 + minutes.parse::<i32>().ok()? * 60))
}

pub(super) fn sleep_clock_times(
    context: &mut Context<'_>,
    start_field: &str,
    end_field: &str,
    profile: &Profile,
) -> Option<OneRecordResult> {
    let start = context.lookup(start_field).and_then(Value::as_str).and_then(parse_millis);
    let end = context.lookup(end_field).and_then(Value::as_str).and_then(parse_millis);
    if start.is_none() {
        context.missing.push(start_field.to_string());
    }
    if end.is_none() {
        context.missing.push(end_field.to_string());
    }
    let (Some(start), Some(end)) = (start, end) else {
        return None;
    };
    let midpoint = start + (end - start) / 2;
    let format = "%Y-%m-%dT%H:%M:%S%:z";
    let profile_zone = profile.time_zone.as_deref().and_then(|name| name.parse::<Tz>().ok().map(|zone| (zone, name)));
    let source_offset = context
        .data
        .get("input_start_timestamp")
        .and_then(|receipt| receipt.get("utc_offset_text"))
        .and_then(Value::as_str)
        .and_then(fixed_offset);
    let local = |at: i64| -> String {
        let instant = Utc.timestamp_millis_opt(at).single().unwrap_or_default();
        match (&profile_zone, source_offset) {
            (Some((zone, _)), _) => instant.with_timezone(zone).format(format).to_string(),
            (None, Some(offset)) => instant.with_timezone(&offset).format(format).to_string(),
            (None, None) => instant.format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
        }
    };
    let (zone_name, basis) = match (&profile_zone, source_offset) {
        (Some((_, name)), _) => (name.to_string(), "athlete_profile.default_time_zone"),
        (None, Some(offset)) => (offset.to_string(), "source offset"),
        (None, None) => ("UTC".to_string(), "utc"),
    };
    Some(OneRecordResult::ClockTimes(ClockTimesResult {
        start_at: time::format_utc_millis(start),
        midpoint_at: time::format_utc_millis(midpoint),
        end_at: time::format_utc_millis(end),
        local_start: local(start),
        local_midpoint: local(midpoint),
        local_end: local(end),
        time_zone: zone_name,
        time_zone_basis: basis,
        duration_hours: round_to((end - start) as f64 / 3_600_000.0, 2),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_durations_cap_gaps_and_end_at_the_workout_end() {
        let samples = [(0, 100.0), (1_000, 110.0), (200_000, 120.0)];
        let (durations, uncovered) = covered_durations(&samples, Some(205_000), 60_000);
        assert_eq!(durations, vec![(100.0, 1_000), (110.0, 60_000), (120.0, 5_000)]);
        assert_eq!(uncovered, 199_000 - 60_000);
        let (_, before) = covered_durations(&[(10_000, 90.0)], Some(20_000), 60_000);
        assert_eq!(before, 10_000, "time before the first sample is uncovered");
    }

    #[test]
    fn heart_rate_is_interpolated_only_across_short_gaps() {
        let samples = [(0, 160.0), (10_000, 150.0), (60_000, 120.0)];
        assert_eq!(heart_rate_at(&samples, 5_000, 10_000, 5_000), Some(155.0));
        assert_eq!(heart_rate_at(&samples, 30_000, 10_000, 5_000), None, "a 50 s gap is not interpolated");
        assert_eq!(heart_rate_at(&samples, 63_000, 10_000, 5_000), Some(120.0), "nearest within tolerance");
    }

    #[test]
    fn units_and_scales_are_recognised_exactly() {
        assert_eq!(kilograms_per("LBS"), Some(POUND_IN_KG));
        assert_eq!(kilograms_per(" kg "), Some(1.0));
        assert_eq!(kilograms_per("stone"), None);
        assert_eq!(effort_scale("0-10"), Some(("10-point", 0.0, 10.0)));
        assert_eq!(effort_scale("CR10"), Some(("10-point", 0.0, 10.0)));
        assert_eq!(effort_scale("6 - 20"), Some(("6-20", 6.0, 20.0)));
        assert_eq!(effort_scale("1-5"), None);
    }

    #[test]
    fn source_offsets_read_as_fixed_offsets() {
        assert_eq!(fixed_offset("Z"), FixedOffset::east_opt(0));
        assert_eq!(fixed_offset("-05:30"), FixedOffset::east_opt(-(5 * 3600 + 30 * 60)));
        assert_eq!(fixed_offset("0500"), None);
    }
}
