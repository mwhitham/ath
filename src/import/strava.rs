//! Strava bulk account exports.
//!
//! `activities.csv` lists one activity per row. Each row becomes one workout
//! keyed by its Activity ID, or by its start when it has none. A row's
//! `Filename` names the activity file inside the export. When that file is a
//! GPX track, its route and heart-rate samples join the workout. A file
//! outside the export is never read: a name no file in the export ends with
//! is left out with a line that says so.
//!
//! Columns the reader does not map stay in `supplied_fields` under their own
//! names. Every file other than `activities.csv` and the activity files is
//! left out as not a training or recovery file.
//!
//! These are the rules of `crates/ath-core/src/strava.rs` in 2.1.0.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::json::{Map, Value};
use crate::model::RecordType;

use super::garmin::{untimed_line, workout_from_gpx};
use super::provider::{CandidateSink, ReportKind, Route, RouteReader};
use super::shared::{
    add_seconds, keyed_candidate, normalize_timestamp, provider_basis, timestamp_receipt, SharedFiles,
    FILE_ROUTE_POLICY,
};

/// A Strava bulk export: a folder or zip archive with `activities.csv`.
pub struct StravaReader {
    pub path: PathBuf,
}

impl StravaReader {
    pub fn new(path: &Path) -> StravaReader {
        StravaReader { path: path.to_path_buf() }
    }
}

/// Columns read into typed fields. Every other column is a supplied field.
const MAPPED_COLUMNS: [&str; 8] = [
    "Activity ID",
    "Activity Date",
    "Activity Name",
    "Activity Type",
    "Elapsed Time",
    "Activity Description",
    "Notes",
    "Filename",
];

impl RouteReader for StravaReader {
    fn route(&self) -> Route {
        Route {
            provider: "strava".into(),
            label: "Strava export".into(),
            vendor_label: "strava".into(),
            data_route: "strava_bulk".into(),
            container: String::new(),
            match_key_method: "strava_bulk/v1".into(),
            policy: FILE_ROUTE_POLICY,
            report: ReportKind::Shared { source: "strava".into() },
        }
    }

    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()> {
        let files = SharedFiles::open(&self.path)?;
        let names = files.names();
        let csv_name = names
            .iter()
            .find(|name| base_name(name) == "activities.csv")
            .ok_or_else(|| Error::refused("Strava export is missing activities.csv"))?;
        let csv = String::from_utf8_lossy(&files.read(csv_name)?).into_owned();
        let source_id = sink.route_source("strava", "strava", None);
        let received_at = sink.received_at().to_string();
        sink.understood("start_at");
        sink.understood("end_at");
        let rows = parse_csv(&csv);
        let Some((header, body)) = rows.split_first() else {
            return Err(Error::refused("activities.csv is empty"));
        };
        for row in body {
            if row.iter().all(String::is_empty) {
                continue;
            }
            let record = row_map(header, row);
            let activity_id = record.get("Activity ID").cloned().unwrap_or_default();
            let start = record
                .get("Activity Date")
                .map(|value| parse_activity_date(value))
                .transpose()
                .map_err(Error::refused)?
                .ok_or_else(|| Error::refused("a Strava row is missing Activity Date"))?;
            let elapsed: f64 = record.get("Elapsed Time").and_then(|value| value.parse().ok()).unwrap_or(0.0);
            let end = add_seconds(&start, elapsed).map_err(Error::refused)?;
            let mut data = Map::new();
            data.insert("source_id".into(), Value::String(source_id.clone()));
            data.insert("received_at".into(), Value::String(received_at.clone()));
            data.insert("received_at_basis".into(), Value::String("import_time".into()));
            data.insert("start_at".into(), Value::String(start.clone()));
            data.insert("end_at".into(), Value::String(end.clone()));
            data.insert("input_start_timestamp".into(), timestamp_receipt(&start));
            data.insert("input_end_timestamp".into(), timestamp_receipt(&end));
            data.insert("source_reported_summaries".into(), Value::Array(Vec::new()));
            if let Some(name) = record.get("Activity Name") {
                data.insert("name".into(), Value::String(name.clone()));
                sink.understood("name");
            }
            if let Some(kind) = record.get("Activity Type") {
                data.insert("activity_type".into(), Value::String(kind.clone()));
                sink.understood("activity_type");
            }
            if let Some(notes) = record.get("Activity Description").or_else(|| record.get("Notes")) {
                if !notes.is_empty() {
                    data.insert("text_result".into(), Value::String(notes.clone()));
                    sink.understood("text_result");
                }
            }
            if !activity_id.is_empty() {
                data.insert("provider_record_id".into(), Value::String(activity_id.clone()));
            }
            if let Some(filename) = record.get("Filename").filter(|value| !value.is_empty()) {
                match names.iter().find(|name| name.ends_with(filename.as_str())) {
                    Some(name) => {
                        if filename.to_ascii_lowercase().ends_with(".gpx") {
                            let gpx = String::from_utf8_lossy(&files.read(name)?).into_owned();
                            let track =
                                workout_from_gpx(&source_id, filename, &gpx, &received_at).map_err(Error::refused)?;
                            if track.untimed > 0 {
                                sink.leave_out(untimed_line(filename, track.untimed));
                            }
                            if let Some(route) = track.data.get("route") {
                                data.insert("route".into(), route.clone());
                                sink.understood("route");
                            }
                            if let Some(samples) = track.data.get("heart_rate_samples") {
                                data.insert("heart_rate_samples".into(), samples.clone());
                                sink.understood("heart_rate_samples");
                            }
                        }
                    }
                    None => sink.leave_out(format!("{filename}: named in activities.csv and not in the export")),
                }
            }
            let mut supplied = Map::new();
            for (key, value) in &record {
                if MAPPED_COLUMNS.contains(&key.as_str()) {
                    continue;
                }
                supplied.insert(key.clone(), Value::String(value.clone()));
                sink.preserved(key.clone());
            }
            if !supplied.is_empty() {
                data.insert("supplied_fields".into(), Value::Object(supplied));
            }
            let has_activity_type = data.contains_key("activity_type");
            data.insert("field_basis".into(), provider_basis(has_activity_type));
            let key = if activity_id.is_empty() { start.clone() } else { activity_id.clone() };
            sink.begin_group(key.clone());
            sink.add(keyed_candidate(RecordType::WorkoutSession, data, &key)?)?;
        }
        for name in names {
            if base_name(name) == "activities.csv" || name.contains("activities/") {
                continue;
            }
            sink.leave_out(format!("{name}: not a training or recovery file"));
        }
        Ok(())
    }
}

fn base_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// A row by column name. A column with an empty name is dropped, and a
/// missing cell is empty text.
fn row_map(header: &[String], row: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (index, key) in header.iter().enumerate() {
        if key.is_empty() {
            continue;
        }
        out.insert(key.clone(), row.get(index).cloned().unwrap_or_default());
    }
    out
}

/// Strava's CSV: lines that are not blank, split on commas outside quotes,
/// with each cell trimmed. A quote only switches quoting on and off.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    text.lines().filter(|line| !line.trim().is_empty()).map(split_csv_line).collect()
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in line.chars() {
        match character {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

/// An Activity Date: an RFC 3339 time, or Strava's `Jan 3, 2026, 4:00:00 PM`,
/// which Strava writes in UTC.
fn parse_activity_date(text: &str) -> std::result::Result<String, String> {
    if let Ok(parsed) = normalize_timestamp(text) {
        return Ok(parsed);
    }
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let unreadable = || format!("could not read Activity Date {text}");
    let mut parts = text.split_whitespace();
    let month = parts.next().unwrap_or("");
    let day = parts.next().unwrap_or("").trim_end_matches(',');
    let year = parts.next().unwrap_or("").trim_end_matches(',');
    let time = parts.next().unwrap_or("");
    let ampm = parts.next().unwrap_or("");
    let month_index = MONTHS.iter().position(|name| *name == month).ok_or_else(unreadable)?;
    let mut clock = time.split(':');
    let (hour_text, minute_text, second_text) =
        (clock.next().unwrap_or("0"), clock.next().unwrap_or("0"), clock.next().unwrap_or("0"));
    let mut hour: u32 = hour_text.parse().map_err(|_| unreadable())?;
    if ampm.eq_ignore_ascii_case("PM") && hour < 12 {
        hour += 12;
    }
    if ampm.eq_ignore_ascii_case("AM") && hour == 12 {
        hour = 0;
    }
    let day: u32 = day.parse().map_err(|_| unreadable())?;
    let year: i32 = year.parse().map_err(|_| unreadable())?;
    let minute: u32 = minute_text.parse().unwrap_or(0);
    let second: u32 = second_text.parse().unwrap_or(0);
    Ok(format!("{year:04}-{:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z", month_index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_dates_read_both_spellings() {
        assert_eq!(parse_activity_date("2026-01-03T16:00:00.000Z").unwrap(), "2026-01-03T16:00:00.000Z");
        assert_eq!(parse_activity_date("Jan 3, 2026, 4:00:00 PM").unwrap(), "2026-01-03T16:00:00.000Z");
        assert_eq!(parse_activity_date("Dec 31, 2025, 12:05:09 AM").unwrap(), "2025-12-31T00:05:09.000Z");
        assert_eq!(parse_activity_date("Smarch 3, 2026").unwrap_err(), "could not read Activity Date Smarch 3, 2026");
    }

    #[test]
    fn quotes_group_commas_and_cells_are_trimmed() {
        assert_eq!(split_csv_line("a, \"b,c\" ,d"), ["a", "b,c", "d"]);
        assert_eq!(parse_csv("h1,h2\r\n\r\n1,2\r\n").len(), 2);
    }
}
