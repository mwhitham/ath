//! Strava bulk account export.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::garmin::{read_files, workout_from_gpx};
use crate::store::{timestamp_receipt, utc_now, Store};

pub fn import_strava(store: &mut Store, path: &Path, write: bool) -> Result<Value, String> {
    let files = read_files(path)?;
    let csv = files
        .iter()
        .find(|(name, _)| name.rsplit('/').next().unwrap_or(name) == "activities.csv")
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
        .ok_or("Strava export is missing activities.csv")?;
    let source_id = if write {
        store.ensure_source("strava", "strava_bulk", "strava")?
    } else {
        "preview".into()
    };
    let received_at = utc_now();
    let mut understood: Vec<String> = vec!["start_at".into(), "end_at".into()];
    let mut preserved = Vec::new();
    let mut left_out = Vec::new();
    let mut record_ids = Vec::new();
    let rows = parse_csv(&csv);
    let Some((header, body)) = rows.split_first() else {
        return Err("activities.csv is empty".into());
    };
    for row in body {
        if row.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        let record = row_map(header, row);
        let activity_id = record.get("Activity ID").cloned().unwrap_or_default();
        let start = record
            .get("Activity Date")
            .map(|value| parse_activity_date(value))
            .transpose()?
            .ok_or("a Strava row is missing Activity Date")?;
        let elapsed: f64 = record
            .get("Elapsed Time")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0);
        let end = crate::garmin::add_seconds(&start, elapsed)?;
        let mut data = Map::new();
        data.insert("source_id".into(), json!(source_id));
        data.insert("received_at".into(), json!(received_at));
        data.insert("received_at_basis".into(), json!("import_time"));
        data.insert("start_at".into(), json!(start));
        data.insert("end_at".into(), json!(end));
        data.insert("input_start_timestamp".into(), timestamp_receipt(&start));
        data.insert("input_end_timestamp".into(), timestamp_receipt(&end));
        data.insert("source_reported_summaries".into(), json!([]));
        if let Some(name) = record.get("Activity Name") {
            data.insert("name".into(), json!(name));
            understood.push("name".into());
        }
        if let Some(kind) = record.get("Activity Type") {
            data.insert("activity_type".into(), json!(kind));
            understood.push("activity_type".into());
        }
        if let Some(notes) = record.get("Activity Description").or_else(|| record.get("Notes")) {
            if !notes.is_empty() {
                data.insert("text_result".into(), json!(notes));
                understood.push("text_result".into());
            }
        }
        if !activity_id.is_empty() {
            data.insert("provider_record_id".into(), json!(activity_id));
        }
        if let Some(filename) = record.get("Filename").filter(|value| !value.is_empty()) {
            if let Some((_, bytes)) = files.iter().find(|(name, _)| name.ends_with(filename)) {
                if filename.to_ascii_lowercase().ends_with(".gpx") {
                    let (gpx_data, _, _, skipped_untimed) = workout_from_gpx(
                        &source_id,
                        filename,
                        &String::from_utf8_lossy(bytes),
                        &received_at,
                    )?;
                    if skipped_untimed > 0 {
                        let label = if skipped_untimed == 1 { "point" } else { "points" };
                        left_out.push(format!(
                            "{filename}: {skipped_untimed} track {label} had no time"
                        ));
                    }
                    if let Some(route) = gpx_data.get("route") {
                        data.insert("route".into(), route.clone());
                        understood.push("route".into());
                    }
                    if let Some(samples) = gpx_data.get("heart_rate_samples") {
                        data.insert("heart_rate_samples".into(), samples.clone());
                        understood.push("heart_rate_samples".into());
                    }
                }
            } else {
                left_out.push(format!("{filename}: named in activities.csv and not in the export"));
            }
        }
        let mut supplied = Map::new();
        for (key, value) in &record {
            if matches!(
                key.as_str(),
                "Activity ID" | "Activity Date" | "Activity Name" | "Activity Type" | "Elapsed Time"
                    | "Activity Description" | "Notes" | "Filename"
            ) {
                continue;
            }
            supplied.insert(key.clone(), json!(value));
            preserved.push(key.clone());
        }
        if !supplied.is_empty() {
            data.insert("supplied_fields".into(), Value::Object(supplied));
        }
        let key = if activity_id.is_empty() {
            start.clone()
        } else {
            activity_id.clone()
        };
        if write {
            let id = store.insert_record(
                "workout_session",
                &data,
                Some(("strava_bulk/v1", &key)),
                "provider_import",
            )?;
            record_ids.push(id);
        }
    }
    for (name, _) in &files {
        let base = name.rsplit('/').next().unwrap_or(name);
        if base == "activities.csv" || name.contains("activities/") {
            continue;
        }
        left_out.push(format!("{name}: not a training or recovery file"));
    }
    understood.sort();
    understood.dedup();
    preserved.sort();
    preserved.dedup();
    Ok(json!({
        "source": "strava",
        "record_ids": record_ids,
        "understood": understood,
        "preserved": preserved,
        "left_out": left_out,
    }))
}

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

fn parse_csv(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(split_csv_line)
        .collect()
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

fn parse_activity_date(text: &str) -> Result<String, String> {
    if let Ok(parsed) = crate::garmin::normalize_timestamp(text) {
        return Ok(parsed);
    }
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = text.split_whitespace();
    let month = parts.next().unwrap_or("");
    let day = parts.next().unwrap_or("").trim_end_matches(',');
    let year = parts.next().unwrap_or("").trim_end_matches(',');
    let time = parts.next().unwrap_or("");
    let ampm = parts.next().unwrap_or("");
    let month_index = months
        .iter()
        .position(|name| *name == month)
        .ok_or_else(|| format!("could not read Activity Date {text}"))?;
    let (hour_text, minute_text, second_text) = {
        let mut clock = time.split(':');
        (
            clock.next().unwrap_or("0"),
            clock.next().unwrap_or("0"),
            clock.next().unwrap_or("0"),
        )
    };
    let mut hour: u32 = hour_text.parse().map_err(|_| format!("could not read Activity Date {text}"))?;
    if ampm.eq_ignore_ascii_case("PM") && hour < 12 {
        hour += 12;
    }
    if ampm.eq_ignore_ascii_case("AM") && hour == 12 {
        hour = 0;
    }
    let day: u32 = day.parse().map_err(|_| format!("could not read Activity Date {text}"))?;
    let year: i32 = year.parse().map_err(|_| format!("could not read Activity Date {text}"))?;
    let minute: u32 = minute_text.parse().unwrap_or(0);
    let second: u32 = second_text.parse().unwrap_or(0);
    Ok(format!(
        "{year:04}-{:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z",
        month_index + 1
    ))
}
