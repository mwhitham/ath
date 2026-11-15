//! WHOOP's dashboard export: four flat CSV files.
//!
//! `physiological_cycles.csv` gives one row per physiological cycle. WHOOP
//! assigns each cycle and the values measured in it; a cycle is not a calendar
//! day, so every value keeps its cycle as a provider reporting period. The
//! cycle's start, end, and offset text are kept as WHOOP wrote them.
//!
//! - HRV, resting heart rate, respiratory rate, skin temperature, and blood
//!   oxygen are point measurements at wake onset. WHOOP's HRV column does not
//!   name its statistic. WHOOP documents it as RMSSD, so it is stored as
//!   `hrv_rmssd` with the `provider_documented_rmssd` basis (D22).
//! - Recovery and day strain are provider scores, not measurements. Recovery
//!   uses the scale `0-100`; strain uses `0-21` and names no unit.
//! - `sleeps.csv` gives sleep sessions, and `workouts.csv` gives workout
//!   sessions with an activity-strain score at the workout start.
//! - `journal_entries.csv` answers are what the athlete said. They become
//!   athlete reports, or nutrition entries for food and drink questions. They
//!   claim no source.
//!
//! WHOOP writes local times in one column and the cycle's UTC offset in
//! another. A row whose time cannot be placed is counted as skipped with an
//! example; it is never read in the computer's own timezone. Any other CSV in
//! the export is counted as an unrecognized file.

use chrono::DateTime;

use crate::error::Result;
use crate::model::*;

use super::candidate::{
    source_reported_summaries, source_value, value_basis, value_field_basis, Aggregate, BasisFacts, Candidate,
    MatchKey, Section, SourceInput,
};
use super::csv::{self, cell, number, positive, Row};
use super::detect::{self, DetectedExport};
use super::instant::normalized_utc;
use super::provider::{CandidateSink, ProviderReader};

/// The WHOOP CSV route.
pub struct WhoopReader;

const KNOWN_FILES: [&str; 4] = ["physiological_cycles.csv", "sleeps.csv", "workouts.csv", "journal_entries.csv"];

impl ProviderReader for WhoopReader {
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()> {
        let files = detect::read_csv_bundle(export)?;
        let source_id = sink.export_source("WHOOP", "whoop", None);
        let text = |name: &str| files.iter().find(|(file, _)| file == name).map(|(_, text)| text.as_str());
        let mut reader = Reader { sink, source_id };
        if let Some(text) = text("physiological_cycles.csv") {
            reader.cycles(&csv::parse_csv(text))?;
        }
        if let Some(text) = text("sleeps.csv") {
            reader.sleeps(&csv::parse_csv(text))?;
        }
        if let Some(text) = text("workouts.csv") {
            reader.workouts(&csv::parse_csv(text))?;
        }
        if let Some(text) = text("journal_entries.csv") {
            reader.journal(&csv::parse_csv(text))?;
        }
        for (name, _) in &files {
            if !KNOWN_FILES.contains(&name.as_str()) {
                reader.sink.skip(&format!("unrecognized WHOOP file: {name}"), name);
            }
        }
        Ok(())
    }
}

struct Reader<'s, 'a> {
    sink: &'s mut CandidateSink<'a>,
    source_id: String,
}

/// The cycle facts every row carries.
struct RowTime {
    offset: Option<String>,
    periods: Option<Vec<ProviderReportingPeriod>>,
}

impl RowTime {
    fn of(row: &Row) -> RowTime {
        let offset = row_offset(row);
        RowTime { periods: cycle_period(row, offset.as_deref()).map(|period| vec![period]), offset }
    }
}

/// A WHOOP score: recovery, day strain, or activity strain.
struct Score<'a> {
    metric: &'a str,
    value: f64,
    scale: &'a str,
    input: SourceInput,
}

const MEASUREMENT_COLUMNS: [(&[&str], PointMeasurementType, &str); 5] = [
    (&["Heart rate variability (ms)", "hrv_ms"], PointMeasurementType::HrvRmssd, "ms"),
    (&["Resting heart rate (bpm)"], PointMeasurementType::RestingHeartRate, "bpm"),
    (&["Respiratory rate (rpm)"], PointMeasurementType::RespiratoryRate, "brpm"),
    (&["Skin temp (celsius)", "skin_temp_celsius"], PointMeasurementType::SkinTemperature, "°C"),
    (&["Blood oxygen %"], PointMeasurementType::OxygenSaturation, "%"),
];

impl Reader<'_, '_> {
    fn basis(&self, record_type: RecordType, quantity: Option<&str>) -> FieldBasis {
        value_field_basis(value_basis(BasisFacts {
            data_route: &self.sink.route().data_route,
            record_type,
            quantity,
            derived: false,
            manual: false,
        }))
    }

    fn score(&mut self, time: &RowTime, at: (&str, &str), score: Score<'_>) -> Result<()> {
        let (text, recorded_at) = at;
        let Score { metric, value, scale, input } = score;
        let recorded = normalized_utc(recorded_at)?;
        let data = ProviderScoreData {
            source_id: self.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: time.periods.clone(),
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::ProviderScore, None)),
            metric: metric.to_string(),
            value: source_value(value, scale, Some(&input)),
            scale: scale.to_string(),
            recorded_at: Some(recorded.clone()),
            input_timestamp: timestamp_receipt(text, time.offset.as_deref()),
        };
        self.sink.add(Candidate::new(
            RecordData::ProviderScore(data),
            MatchKey::measured(RecordType::ProviderScore, metric, &recorded),
            Section::Record,
        ))
    }

    fn cycles(&mut self, rows: &[Row]) -> Result<()> {
        for row in rows {
            let time = RowTime::of(row);
            let wake_text = cell(row, &["Wake onset", "Cycle end time", "Cycle start time"]);
            let Some(wake) = with_offset(wake_text.as_deref(), time.offset.as_deref()) else {
                self.sink.skip(
                    "cycle rows with an unreadable timestamp",
                    &describe_time(wake_text.as_deref(), time.offset.as_deref()),
                );
                continue;
            };
            let wake_text = wake_text.unwrap_or_default();
            for (columns, quantity, unit) in MEASUREMENT_COLUMNS {
                let Some(value) = positive(row, columns) else {
                    continue;
                };
                let recorded = normalized_utc(&wake)?;
                let data = PointMeasurementData {
                    source_id: self.source_id.clone(),
                    provider_record_id: None,
                    provider_reporting_periods: time.periods.clone(),
                    received_at: self.sink.received_at().to_string(),
                    received_at_basis: ReceivedAtBasis::ImportTime,
                    supplied_fields: None,
                    field_basis: Some(self.basis(RecordType::PointMeasurement, Some(quantity.as_str()))),
                    quantity,
                    recorded_at: Some(recorded.clone()),
                    input_timestamp: timestamp_receipt(&wake_text, time.offset.as_deref()),
                    value: source_value(round2(value), unit, Some(&SourceInput::written(value, unit))),
                    calculations: None,
                };
                self.sink.add(Candidate::new(
                    RecordData::PointMeasurement(data),
                    MatchKey::measured(RecordType::PointMeasurement, quantity.as_str(), &recorded),
                    Section::Record,
                ))?;
            }
            if let Some(recovery) = number(row, &["Recovery score %", "recovery_score"]) {
                let input = SourceInput::written(recovery, "%");
                self.score(
                    &time,
                    (&wake_text, &wake),
                    Score { metric: "recovery", value: recovery, scale: "0-100", input },
                )?;
            }
            if let Some(strain) = number(row, &["Day Strain", "day_strain"]) {
                let input = SourceInput::without_unit(strain);
                self.score(
                    &time,
                    (&wake_text, &wake),
                    Score { metric: "strain", value: round2(strain), scale: "0-21", input },
                )?;
            }
        }
        Ok(())
    }

    fn sleeps(&mut self, rows: &[Row]) -> Result<()> {
        for row in rows {
            let time = RowTime::of(row);
            let start_text = cell(row, &["Sleep onset"]);
            let end_text = cell(row, &["Wake onset"]);
            let Some((start, end)) = time_range(start_text.as_deref(), end_text.as_deref(), time.offset.as_deref())
            else {
                self.sink.skip(
                    "sleep rows with an unusable time range",
                    &range_example(start_text.as_deref(), end_text.as_deref(), time.offset.as_deref()),
                );
                continue;
            };
            let mut aggregates = Vec::new();
            let mut minutes = |name: &str, columns: &[&str]| {
                if let Some(value) = positive(row, columns) {
                    aggregates.push(Aggregate {
                        name: name.into(),
                        value: (value * 60.0).round(),
                        input: Some(SourceInput::written(value, "min")),
                    });
                }
            };
            minutes("duration_s", &["Asleep duration (min)"]);
            minutes("time_in_bed_s", &["In bed duration (min)"]);
            minutes("light_s", &["Light sleep duration (min)"]);
            minutes("deep_s", &["Deep (SWS) duration (min)", "deep_sleep_duration_min"]);
            minutes("rem_s", &["REM duration (min)"]);
            minutes("awake_s", &["Awake duration (min)"]);
            if let Some(efficiency) = positive(row, &["Sleep efficiency %"]) {
                aggregates.push(Aggregate {
                    name: "efficiency_pct".into(),
                    value: ((efficiency * 10.0).round() / 10.0).min(100.0),
                    input: Some(SourceInput::written(efficiency, "%")),
                });
            }
            let start_at = normalized_utc(&start)?;
            let data = SleepSessionData {
                source_id: self.source_id.clone(),
                provider_record_id: None,
                provider_reporting_periods: time.periods.clone(),
                received_at: self.sink.received_at().to_string(),
                received_at_basis: ReceivedAtBasis::ImportTime,
                supplied_fields: None,
                field_basis: Some(self.basis(RecordType::SleepSession, None)),
                start_at: start_at.clone(),
                end_at: normalized_utc(&end)?,
                input_start_timestamp: timestamp_receipt(&start_text.unwrap_or_default(), time.offset.as_deref()),
                input_end_timestamp: timestamp_receipt(&end_text.unwrap_or_default(), time.offset.as_deref()),
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
            ))?;
        }
        Ok(())
    }

    fn workouts(&mut self, rows: &[Row]) -> Result<()> {
        for row in rows {
            let time = RowTime::of(row);
            let start_text = cell(row, &["Workout start time"]);
            let end_text = cell(row, &["Workout end time"]);
            let Some((start, end)) = time_range(start_text.as_deref(), end_text.as_deref(), time.offset.as_deref())
            else {
                self.sink.skip(
                    "workout rows with an unusable time range",
                    &range_example(start_text.as_deref(), end_text.as_deref(), time.offset.as_deref()),
                );
                continue;
            };
            let activity = cell(row, &["Activity name"]).map(|name| name.to_lowercase());
            let mut aggregates = Vec::new();
            for (name, columns, unit) in [
                ("avg_hr_bpm", &["Average HR (bpm)", "average_hr_bpm"][..], "bpm"),
                ("max_hr_bpm", &["Max HR (bpm)", "max_hr_bpm"][..], "bpm"),
                ("energy_kcal", &["Energy burned (cal)"][..], "cal"),
                ("distance_m", &["Distance (meters)", "distance_meters"][..], "m"),
            ] {
                if let Some(value) = positive(row, columns) {
                    aggregates.push(Aggregate {
                        name: name.into(),
                        value: value.round(),
                        input: Some(SourceInput::written(value, unit)),
                    });
                }
            }
            let start_at = normalized_utc(&start)?;
            let mut field_basis = self.basis(RecordType::WorkoutSession, None);
            if activity.is_some() {
                field_basis.activity_type = Some(ActivityTypeBasis::ProviderLabel);
            }
            let data = WorkoutSessionData {
                source_id: self.source_id.clone(),
                provider_record_id: None,
                provider_reporting_periods: time.periods.clone(),
                received_at: self.sink.received_at().to_string(),
                received_at_basis: ReceivedAtBasis::ImportTime,
                supplied_fields: None,
                field_basis: Some(field_basis),
                start_at: start_at.clone(),
                end_at: normalized_utc(&end)?,
                input_start_timestamp: timestamp_receipt(
                    start_text.as_deref().unwrap_or_default(),
                    time.offset.as_deref(),
                ),
                input_end_timestamp: timestamp_receipt(end_text.as_deref().unwrap_or_default(), time.offset.as_deref()),
                activity_type: activity,
                name: None,
                description: None,
                text_result: None,
                maximum_heart_rate_bpm: None,
                resting_heart_rate_bpm: None,
                heart_rate_samples: None,
                sets: None,
                route: None,
                source_reported_summaries: source_reported_summaries(&aggregates),
                segments: None,
            };
            self.sink.add(Candidate::new(
                RecordData::WorkoutSession(data),
                MatchKey::session(RecordType::WorkoutSession, &start_at),
                Section::Record,
            ))?;
            if let Some(strain) = number(row, &["Activity Strain", "activity_strain"]) {
                let text = start_text.clone().unwrap_or_default();
                let input = SourceInput::without_unit(strain);
                let score = Score { metric: "activity_strain", value: round2(strain), scale: "0-21", input };
                self.score(&time, (&text, &start), score)?;
            }
        }
        Ok(())
    }

    fn journal(&mut self, rows: &[Row]) -> Result<()> {
        for row in rows {
            let offset = row_offset(row);
            let reported_text = cell(row, &["Cycle end time", "Cycle start time"]);
            let reported = with_offset(reported_text.as_deref(), offset.as_deref());
            let question = cell(row, &["Question text", "question_text"]);
            let (Some(reported), Some(question)) = (reported, question.clone()) else {
                let example = if question.is_none() {
                    "row has no question text".to_string()
                } else if reported_text.is_none() {
                    "row has neither a cycle start nor end time".to_string()
                } else {
                    describe_time(reported_text.as_deref(), offset.as_deref())
                };
                self.sink.skip("journal rows we cannot place in time", &example);
                continue;
            };
            let answer = cell(row, &["Answered yes", "answered_yes", "Answer"]).unwrap_or_default();
            let affirmative = matches!(answer.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1");
            let mut note = format!("{question}: {}", if affirmative { "yes" } else { "no" });
            if let Some(notes) = cell(row, &["Notes"]) {
                note = format!("{note} — {notes}");
            }
            let kind = journal_kind(&question);
            let reported_at = normalized_utc(&reported)?;
            let receipt = timestamp_receipt(&reported_text.unwrap_or_default(), offset.as_deref());
            let data = match kind {
                JournalKind::Nutrition => RecordData::NutritionEntry(NutritionEntryData {
                    reported_at: reported_at.clone(),
                    input_reported_at: receipt,
                    reported_via: ReportedVia::Text,
                    athlete_description: note,
                    reported_items: Vec::new(),
                    attachment_ids: None,
                }),
                JournalKind::Report(report_type) => RecordData::AthleteReport(AthleteReportData {
                    report_type,
                    reported_at: reported_at.clone(),
                    input_reported_at: receipt,
                    reported_via: ReportedVia::Text,
                    rating: None,
                    rating_scale: None,
                    body_region: None,
                    note: Some(note),
                    duration_seconds: None,
                    attachment_ids: None,
                }),
            };
            self.sink.add(Candidate::new(
                data,
                MatchKey::athlete_report(kind.as_str(), &reported_at),
                Section::Report,
            ))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalKind {
    Nutrition,
    Report(AthleteReportType),
}

impl JournalKind {
    fn as_str(self) -> &'static str {
        match self {
            JournalKind::Nutrition => "nutrition",
            JournalKind::Report(report_type) => report_type.as_str(),
        }
    }
}

/// The kind of a journal answer, from words in its question.
fn journal_kind(question: &str) -> JournalKind {
    let lower = question.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|word| lower.contains(word));
    if any(&["alcohol", "caffeine", "coffee", "ate ", "eat ", "meal", "hydrat", "water", "supplement", "creatine"]) {
        JournalKind::Nutrition
    } else if any(&["stress", "anxious", "anxiety", "meditat", "breathwork", "journal"]) {
        JournalKind::Report(AthleteReportType::Stress)
    } else if any(&["sleep", "bed", "nap", "blue light", "screen"]) {
        JournalKind::Report(AthleteReportType::SleepQuality)
    } else if any(&["sore", "injur", "pain"]) {
        JournalKind::Report(AthleteReportType::Soreness)
    } else if any(&["energy", "fatigue", "tired"]) {
        JournalKind::Report(AthleteReportType::Energy)
    } else if any(&["mood", "happy", "sad"]) {
        JournalKind::Report(AthleteReportType::Mood)
    } else {
        JournalKind::Report(AthleteReportType::Note)
    }
}

fn row_offset(row: &Row) -> Option<String> {
    cell(row, &["Cycle timezone", "cycle_timezone", "timezone_offset"])
}

/// The cycle as a provider reporting period, when its start can be placed.
fn cycle_period(row: &Row, offset: Option<&str>) -> Option<ProviderReportingPeriod> {
    let input_start = cell(row, &["Cycle start time"]);
    let input_end = cell(row, &["Cycle end time"]);
    let start = with_offset(input_start.as_deref(), offset)?;
    let end = input_end.as_deref().and_then(|_| with_offset(input_end.as_deref(), offset));
    Some(ProviderReportingPeriod {
        period_type: ProviderReportingPeriodType::WhoopPhysiologicalCycle,
        provider_period_id: None,
        provider_date: None,
        start_at: Some(to_utc(&start)),
        end_at: end.as_deref().map(to_utc),
        input_start_timestamp: Some(timestamp_receipt(input_start.as_deref().unwrap_or_default(), offset)),
        input_end_timestamp: end.as_ref().map(|_| timestamp_receipt(input_end.as_deref().unwrap_or_default(), offset)),
        timezone_offset_text: offset.map(str::to_string),
        timezone_offset_meaning: offset.map(|_| UtcOffsetMeaning::ProviderCycleOffset),
        timezone_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
    })
}

/// The receipt for a WHOOP local time: the text as written, and the cycle's
/// offset text when the row has one.
fn timestamp_receipt(text: &str, offset: Option<&str>) -> InputTimestampReceipt {
    InputTimestampReceipt {
        timestamp_text: Some(text.to_string()),
        utc_offset_text: offset.map(str::to_string),
        utc_offset_meaning: offset.map(|_| UtcOffsetMeaning::ProviderCycleOffset),
        source_time_zone: None,
        source_time_zone_meaning: None,
        timestamp_basis: Some(InputTimestampBasis::SourceLocalTimestampWithDocumentedOffset),
        timestamp_missing_reason: None,
        utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
    }
}

/// Start and end with offsets, when both can be placed and the end is after the start.
fn time_range(start: Option<&str>, end: Option<&str>, offset: Option<&str>) -> Option<(String, String)> {
    let start = with_offset(start, offset)?;
    let end = with_offset(end, offset)?;
    (parse_ms(&end) > parse_ms(&start)).then_some((start, end))
}

fn range_example(start: Option<&str>, end: Option<&str>, offset: Option<&str>) -> String {
    format!("{} → {}", describe_time(start, offset), describe_time(end, offset))
}

/// A local WHOOP time with its offset attached, as ISO 8601 text.
///
/// Text that already ends in an offset or `Z` is kept, with spaces turned into
/// `T`. Otherwise the date and time are taken by position and the row's
/// offset is appended; missing seconds become `00`. Text shorter than
/// `YYYY-MM-DD HH:MM`, or whose year is not four digits, gives nothing.
pub fn with_offset(local: Option<&str>, offset: Option<&str>) -> Option<String> {
    let trimmed = local?.trim();
    if trimmed.len() < 16 {
        return None;
    }
    let part = |range: std::ops::Range<usize>| trimmed.get(range);
    let year = part(0..4)?;
    let month = part(5..7)?;
    let day = part(8..10)?;
    let hour = part(11..13)?;
    let minute = part(14..16)?;
    if !year.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let second = if trimmed.len() >= 19 && trimmed.as_bytes().get(16) == Some(&b':') { part(17..19)? } else { "00" };
    if has_offset_at_end(trimmed) {
        return Some(trimmed.replace(' ', "T"));
    }
    let offset = normalize_offset(offset)?;
    Some(format!("{year}-{month}-{day}T{hour}:{minute}:{second}{offset}"))
}

/// True when text ends in `Z`, `z`, `±HH:MM`, or `±HHMM` after its last `T`.
fn has_offset_at_end(text: &str) -> bool {
    if text.ends_with('Z') || text.ends_with('z') {
        return true;
    }
    let bytes = text.as_bytes();
    for index in (0..bytes.len()).rev() {
        match bytes[index] {
            b'T' => return false,
            b'+' | b'-' => {
                let rest = &bytes[index..];
                if rest.len() >= 6 && rest[3] == b':' {
                    return true;
                }
                if rest.len() >= 5 && rest[1..].iter().all(u8::is_ascii_digit) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Read the several spellings of a UTC offset: `UTC-07:00`, `-0700`, `-07:00`,
/// `+5:30`, and every spelling of zero, including WHOOP's `UTCZ`. Anything
/// else, such as a zone name, gives nothing rather than a guess.
pub fn normalize_offset(offset: Option<&str>) -> Option<String> {
    let trimmed = offset?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "Z" || trimmed.eq_ignore_ascii_case("UTC") || trimmed.eq_ignore_ascii_case("GMT") {
        return Some("Z".into());
    }
    let prefix =
        trimmed.get(..3).filter(|prefix| prefix.eq_ignore_ascii_case("UTC") || prefix.eq_ignore_ascii_case("GMT"));
    let bare = match prefix {
        Some(_) => trimmed[3..].trim_start(),
        None => trimmed,
    };
    if bare.is_empty() || bare == "Z" || bare == "z" {
        return Some("Z".into());
    }
    let bytes = bare.as_bytes();
    let sign = *bytes.first()?;
    if sign != b'+' && sign != b'-' {
        return None;
    }
    let body = &bare[1..];
    let (hours, minutes) = match body.split_once(':') {
        Some((hours, minutes)) => (hours, minutes),
        None if body.len() >= 3 => body.split_at(body.len() - 2),
        None => return None,
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(hours) || hours.len() > 2 || !digits(minutes) || minutes.len() != 2 {
        return None;
    }
    if hours == "00" && minutes == "00" {
        return Some("Z".into());
    }
    Some(format!("{}{hours:0>2}:{minutes}", sign as char))
}

/// The cycle boundary as a stored UTC instant. Text RFC 3339 cannot read is
/// kept as written, so the record fails validation instead of moving in time.
fn to_utc(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.to_utc().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|_| value.to_string())
}

/// Milliseconds for an RFC 3339 time, or 0 when it cannot be read.
fn parse_ms(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value).map(|time| time.timestamp_millis()).unwrap_or(0)
}

fn describe_time(value: Option<&str>, offset: Option<&str>) -> String {
    match value {
        None => "blank timestamp".into(),
        Some(value) => format!("\"{value}\" with timezone \"{}\"", offset.unwrap_or("(none)")),
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_whoop_utc_prefixed_offsets() {
        // The real export writes `UTC-07:00`. An earlier version read only the
        // bare form, so every row of a real export was skipped.
        assert_eq!(normalize_offset(Some("UTC-07:00")).as_deref(), Some("-07:00"));
        assert_eq!(normalize_offset(Some("UTC+02:00")).as_deref(), Some("+02:00"));
        assert_eq!(normalize_offset(Some("UTC+05:30")).as_deref(), Some("+05:30"));
    }

    #[test]
    fn reads_bare_offsets_in_either_punctuation() {
        assert_eq!(normalize_offset(Some("-0700")).as_deref(), Some("-07:00"));
        assert_eq!(normalize_offset(Some("-07:00")).as_deref(), Some("-07:00"));
        assert_eq!(normalize_offset(Some("+01:00")).as_deref(), Some("+01:00"));
        assert_eq!(normalize_offset(Some("+5:30")).as_deref(), Some("+05:30"));
        assert_eq!(normalize_offset(Some("+530")).as_deref(), Some("+05:30"));
    }

    #[test]
    fn every_spelling_of_zero_offset_is_utc() {
        for spelling in ["Z", "UTC", "GMT", "utc", "UTC+00:00", "UTC-00:00", "UTCZ", "GMT+0000", "+00:00", "-0000", "z"]
        {
            assert_eq!(normalize_offset(Some(spelling)).as_deref(), Some("Z"), "{spelling}");
        }
    }

    #[test]
    fn refuses_offsets_it_cannot_read() {
        // A zone name is not an offset: one name means different offsets on
        // different dates, so a guess would silently move timestamps.
        for spelling in ["America/Los_Angeles", "PST", "CET", "", "+ab:cd", "+12345", "UTC+05:30:00", "GMT-7", "5:00"] {
            assert_eq!(normalize_offset(Some(spelling)), None, "{spelling}");
        }
        assert_eq!(normalize_offset(None), None);
    }

    #[test]
    fn combines_a_local_time_with_the_cycle_offset() {
        assert_eq!(
            with_offset(Some("2026-08-09 06:20:00"), Some("UTC-07:00")).as_deref(),
            Some("2026-08-09T06:20:00-07:00")
        );
        assert_eq!(
            with_offset(Some("2026-08-09 06:20"), Some("UTC-07:00")).as_deref(),
            Some("2026-08-09T06:20:00-07:00")
        );
        assert_eq!(with_offset(Some("2026-08-09T06:20:00-07:00"), None).as_deref(), Some("2026-08-09T06:20:00-07:00"));
        assert_eq!(with_offset(Some("2026-08-09T06:20:00Z"), None).as_deref(), Some("2026-08-09T06:20:00Z"));
        assert_eq!(
            with_offset(Some("2026-08-09T18:00:00.000-04:00"), None).as_deref(),
            Some("2026-08-09T18:00:00.000-04:00")
        );
    }

    #[test]
    fn a_blank_or_unreadable_time_gives_nothing() {
        // A local time with no offset is not a valid timestamp in this format.
        assert_eq!(with_offset(Some("2026-08-09 06:20:00"), None), None);
        assert_eq!(with_offset(None, Some("UTC-07:00")), None);
        assert_eq!(with_offset(Some("not a date"), Some("UTC-07:00")), None);
        assert_eq!(with_offset(Some("2026-08-09 06:20:00"), Some("PST")), None);
        // Text that is not cut at character boundaries is unreadable, not a crash.
        assert_eq!(with_offset(Some("2026-0é-09 06:20:00"), Some("UTC")), None);
    }

    #[test]
    fn journal_questions_choose_report_kinds() {
        assert_eq!(journal_kind("Have any alcoholic drinks?"), JournalKind::Nutrition);
        assert_eq!(journal_kind("Practice mediation or breathwork?"), JournalKind::Report(AthleteReportType::Stress));
        assert_eq!(journal_kind("Read in bed?"), JournalKind::Report(AthleteReportType::SleepQuality));
        assert_eq!(journal_kind("Feeling sick or unwell?"), JournalKind::Report(AthleteReportType::Note));
    }
}
