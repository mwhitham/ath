//! Garmin account exports and activity files, and the Garmin personal pull.
//!
//! A Garmin input is one file, a folder, or a zip archive. Each file is read
//! by its ending:
//!
//! - `.fit`: the record messages of an activity become one workout with its
//!   route and heart-rate samples. A FIT file with no timed record message, or
//!   one the decoder cannot read, is left out with the reason.
//! - `.gpx`: the track points become one workout. A track point without a time
//!   is left out and counted. A time without an offset is never read in the
//!   computer's timezone: it is kept as written, and the workout check then
//!   refuses the import, as 2.1.0 refused it when it wrote.
//! - `.json`: each activity becomes one workout. Fields the reader does not
//!   map stay in `supplied_fields` under their own names. Garmin's `maxHR` is
//!   that workout's peak, not the athlete's known maximum, so it becomes a
//!   `max_heart_rate` provider summary and never `maximum_heart_rate_bpm`.
//! - Any other file is left out as not a training or recovery file.
//!
//! A workout's key is its activity ID, or its file name. The summary's key is
//! the workout's key with `:max_heart_rate`. The Garmin pull reads one day of
//! activities and the night's HRV average from the personal connection with
//! the same activity rules; its keys use the method `garmin_personal/v1`.
//!
//! These are the rules of `crates/ath-core/src/garmin.rs` and `personal.rs`
//! in 2.1.0.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::json::{Map, Value};
use crate::model::RecordType;

use super::fit::{read_fit, FitPoint};
use super::provider::{CandidateSink, ReportKind, Route, RouteReader};
use super::shared::{
    add_seconds, format_millis, keyed_candidate, normalize_timestamp, provider_basis, timestamp_receipt, SharedFiles,
    FILE_ROUTE_POLICY,
};

/// Garmin account export files: FIT, GPX, and activity JSON.
pub struct GarminReader {
    pub path: PathBuf,
}

impl GarminReader {
    pub fn new(path: &Path) -> GarminReader {
        GarminReader { path: path.to_path_buf() }
    }
}

impl RouteReader for GarminReader {
    fn route(&self) -> Route {
        Route {
            provider: "garmin".into(),
            label: "Garmin export".into(),
            vendor_label: "garmin".into(),
            data_route: "garmin_account_export".into(),
            container: String::new(),
            match_key_method: "garmin_account/v1".into(),
            policy: FILE_ROUTE_POLICY,
            report: ReportKind::Shared { source: "garmin".into() },
        }
    }

    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()> {
        let source_id = sink.route_source("garmin", "garmin", None);
        let received_at = sink.received_at().to_string();
        let files = SharedFiles::open(&self.path)?;
        for name in files.names() {
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".fit") {
                match read_fit(&files.read(name)?) {
                    Ok(points) if !points.is_empty() => {
                        let (data, understood) = workout_from_fit(&source_id, name, &points, &received_at);
                        understood.into_iter().for_each(|field| sink.understood(field));
                        sink.begin_group(name.clone());
                        sink.add(keyed_candidate(RecordType::WorkoutSession, data, name)?)?;
                    }
                    Ok(_) => sink.leave_out(format!("{name}: FIT file had no record points")),
                    Err(error) => sink.leave_out(format!("{name}: {error}")),
                }
                continue;
            }
            if lower.ends_with(".gpx") {
                let gpx = String::from_utf8_lossy(&files.read(name)?).into_owned();
                let workout = workout_from_gpx(&source_id, name, &gpx, &received_at).map_err(Error::refused)?;
                workout.understood.iter().for_each(|field| sink.understood(field.clone()));
                if workout.untimed > 0 {
                    sink.leave_out(untimed_line(name, workout.untimed));
                }
                sink.begin_group(name.clone());
                sink.add(keyed_candidate(RecordType::WorkoutSession, workout.data, name)?)?;
                continue;
            }
            if lower.ends_with(".json") {
                let value: Value =
                    serde_json::from_slice(&files.read(name)?).map_err(|error| Error::refused(error.to_string()))?;
                for activity in activities_in(&value) {
                    let workout = workout_from_json(&source_id, activity, &received_at).map_err(Error::refused)?;
                    workout.understood.iter().for_each(|field| sink.understood(field.clone()));
                    workout.preserved.iter().for_each(|field| sink.preserved(field.clone()));
                    let key = activity
                        .get("activityId")
                        .or_else(|| activity.get("activity_id"))
                        .map(value_key)
                        .unwrap_or_else(|| name.clone());
                    let max_hr = activity.get("maxHR").and_then(Value::as_f64).filter(|value| *value > 0.0);
                    if max_hr.is_some() {
                        sink.understood("max_heart_rate");
                    }
                    let start_at = workout.data.get("start_at").and_then(Value::as_str).unwrap_or_default().to_string();
                    sink.begin_group(key.clone());
                    sink.add(keyed_candidate(RecordType::WorkoutSession, workout.data, &key)?)?;
                    if let Some(bpm) = max_hr {
                        let summary = max_heart_rate_summary(&source_id, &received_at, &start_at, bpm, &key);
                        sink.add(keyed_candidate(
                            RecordType::ProviderSummary,
                            summary,
                            &format!("{key}:max_heart_rate"),
                        )?)?;
                    }
                }
                continue;
            }
            sink.leave_out(format!("{name}: not a training or recovery file"));
        }
        Ok(())
    }
}

/// `{name}: 1 track point had no time`, or `points` for more than one.
pub fn untimed_line(name: &str, count: usize) -> String {
    let label = if count == 1 { "point" } else { "points" };
    format!("{name}: {count} track {label} had no time")
}

/// A workout as the 2.1.0 helper built it, with the fields it read.
pub struct Workout {
    pub data: Map<String, Value>,
    pub understood: Vec<String>,
    pub preserved: Vec<String>,
    /// Track points left out because they had no time.
    pub untimed: usize,
}

fn base(source_id: &str, received_at: &str, start: &str, end: &str) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert("source_id".into(), Value::String(source_id.into()));
    data.insert("received_at".into(), Value::String(received_at.into()));
    data.insert("received_at_basis".into(), Value::String("import_time".into()));
    data.insert("start_at".into(), Value::String(start.into()));
    data.insert("end_at".into(), Value::String(end.into()));
    data.insert("input_start_timestamp".into(), timestamp_receipt(start));
    data.insert("input_end_timestamp".into(), timestamp_receipt(end));
    data.insert("source_reported_summaries".into(), Value::Array(Vec::new()));
    data
}

/// The `max_heart_rate` summary of one Garmin activity. Its time is the
/// workout's start, and its provider record ID is the workout's key.
fn max_heart_rate_summary(
    source_id: &str,
    received_at: &str,
    recorded_at: &str,
    bpm: f64,
    provider_record_id: &str,
) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert("source_id".into(), Value::String(source_id.into()));
    data.insert("received_at".into(), Value::String(received_at.into()));
    data.insert("received_at_basis".into(), Value::String("import_time".into()));
    data.insert("summary_type".into(), Value::String("max_heart_rate".into()));
    data.insert("recorded_at".into(), Value::String(recorded_at.into()));
    data.insert("input_timestamp".into(), timestamp_receipt(recorded_at));
    data.insert(
        "value".into(),
        serde_json::json!({ "standard_value": bpm, "standard_unit": "bpm", "input_value": bpm, "input_unit": "bpm" }),
    );
    data.insert("provider_record_id".into(), Value::String(provider_record_id.into()));
    data.insert("field_basis".into(), provider_basis(false));
    data
}

/// The activities of a Garmin JSON file: an array, the array under
/// `summarizedActivities`, `activities`, or `activityList`, or the object itself.
fn activities_in(value: &Value) -> Vec<&Map<String, Value>> {
    if let Some(items) = value.as_array() {
        return items.iter().filter_map(Value::as_object).collect();
    }
    if let Some(object) = value.as_object() {
        for key in ["summarizedActivities", "activities", "activityList"] {
            if let Some(items) = object.get(key).and_then(Value::as_array) {
                return items.iter().filter_map(Value::as_object).collect();
            }
        }
        return vec![object];
    }
    Vec::new()
}

/// Fields of a Garmin activity that become typed values or are read for the
/// workout. Every other field stays in `supplied_fields`.
fn mapped_key(key: &str) -> bool {
    matches!(
        key,
        "activityName"
            | "name"
            | "description"
            | "activityType"
            | "activityId"
            | "activity_id"
            | "startTimeGMT"
            | "start_at"
            | "duration"
            | "elapsedDuration"
            | "maxHR"
            | "samples"
            | "heart_rate_samples"
            | "route"
    )
}

/// One Garmin activity as a workout.
pub fn workout_from_json(
    source_id: &str,
    activity: &Map<String, Value>,
    received_at: &str,
) -> std::result::Result<Workout, String> {
    let mut understood: Vec<String> = Vec::new();
    let mut preserved = Vec::new();
    let start = activity
        .get("startTimeGMT")
        .or_else(|| activity.get("start_at"))
        .and_then(Value::as_str)
        .map(normalize_timestamp)
        .transpose()?
        .ok_or("activity is missing startTimeGMT")?;
    let duration =
        activity.get("duration").or_else(|| activity.get("elapsedDuration")).and_then(Value::as_f64).unwrap_or(0.0);
    let end = add_seconds(&start, duration)?;
    let mut data = base(source_id, received_at, &start, &end);
    understood.extend(["start_at".into(), "end_at".into()]);
    if let Some(name) = text_field(activity, &["activityName", "name"]) {
        data.insert("name".into(), Value::String(name));
        understood.push("name".into());
    }
    if let Some(description) = text_field(activity, &["description"]) {
        data.insert("description".into(), Value::String(description));
        understood.push("description".into());
    }
    if let Some(activity_type) = activity_type(activity) {
        data.insert("activity_type".into(), Value::String(activity_type));
        understood.push("activity_type".into());
    }
    if let Some(id) = activity.get("activityId").map(value_key) {
        data.insert("provider_record_id".into(), Value::String(id));
        understood.push("provider_record_id".into());
    }
    if let Some(samples) = activity.get("samples").or_else(|| activity.get("heart_rate_samples")) {
        data.insert("heart_rate_samples".into(), samples.clone());
        understood.push("heart_rate_samples".into());
    }
    if let Some(route) = activity.get("route") {
        data.insert("route".into(), route.clone());
        understood.push("route".into());
    }
    let mut supplied = Map::new();
    for (key, value) in activity {
        if mapped_key(key) {
            continue;
        }
        supplied.insert(key.clone(), value.clone());
        preserved.push(key.clone());
    }
    if !supplied.is_empty() {
        data.insert("supplied_fields".into(), Value::Object(supplied));
    }
    let has_activity_type = data.contains_key("activity_type");
    data.insert("field_basis".into(), provider_basis(has_activity_type));
    Ok(Workout { data, understood, preserved, untimed: 0 })
}

/// One GPX track as a workout. `name` is the file name: the workout's name
/// when the track has none, and its provider record ID.
pub fn workout_from_gpx(
    source_id: &str,
    name: &str,
    gpx: &str,
    received_at: &str,
) -> std::result::Result<Workout, String> {
    let (points, untimed) = gpx_points(gpx);
    if points.is_empty() {
        if untimed > 0 {
            return Err(format!("{name} has no timed track points; {untimed} had no time"));
        }
        return Err(format!("{name} has no track points"));
    }
    let start = points[0].0.clone();
    let end = points[points.len() - 1].0.clone();
    let mut data = base(source_id, received_at, &start, &end);
    data.insert("name".into(), Value::String(track_name(gpx).unwrap_or_else(|| name.to_string())));
    data.insert("provider_record_id".into(), Value::String(name.into()));
    let route: Vec<Value> = points
        .iter()
        .map(
            |(time, lat, lon, _)| serde_json::json!({ "recorded_at": time, "latitude_deg": lat, "longitude_deg": lon }),
        )
        .collect();
    data.insert("route".into(), Value::Array(route));
    let start_ms = parse_time_ms(&points[0].0).unwrap_or(0);
    let samples: Vec<Value> = points
        .iter()
        .filter_map(|(time, _, _, hr)| {
            let bpm = (*hr)?;
            let offset = parse_time_ms(time).unwrap_or(start_ms) - start_ms;
            Some(serde_json::json!({ "offset_ms": offset, "bpm": bpm }))
        })
        .collect();
    let mut understood: Vec<String> = vec!["name".into(), "route".into(), "start_at".into(), "end_at".into()];
    if !samples.is_empty() {
        data.insert("heart_rate_samples".into(), Value::Array(samples));
        understood.push("heart_rate_samples".into());
    }
    data.insert("field_basis".into(), provider_basis(false));
    Ok(Workout { data, understood, preserved: Vec::new(), untimed })
}

/// One FIT activity as a workout, named by its file name.
fn workout_from_fit(
    source_id: &str,
    name: &str,
    points: &[FitPoint],
    received_at: &str,
) -> (Map<String, Value>, Vec<String>) {
    let start_ms = points[0].recorded_at_ms;
    let end_ms = points[points.len() - 1].recorded_at_ms.max(start_ms);
    let mut data = base(source_id, received_at, &format_millis(start_ms), &format_millis(end_ms));
    data.insert("name".into(), Value::String(name.into()));
    data.insert("provider_record_id".into(), Value::String(name.into()));
    let route: Vec<Value> = points
        .iter()
        .filter_map(|point| {
            Some(serde_json::json!({
                "recorded_at": format_millis(point.recorded_at_ms),
                "latitude_deg": point.latitude_deg?,
                "longitude_deg": point.longitude_deg?,
            }))
        })
        .collect();
    if !route.is_empty() {
        data.insert("route".into(), Value::Array(route));
    }
    let samples: Vec<Value> = points
        .iter()
        .filter_map(|point| point.bpm.map(|bpm| serde_json::json!({ "offset_ms": point.offset_ms, "bpm": bpm })))
        .collect();
    if !samples.is_empty() {
        data.insert("heart_rate_samples".into(), Value::Array(samples));
    }
    data.insert("field_basis".into(), provider_basis(false));
    (data, vec!["name".into(), "route".into(), "heart_rate_samples".into()])
}

fn parse_time_ms(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text).ok().map(|time| time.timestamp_millis())
}

/// The name inside the first `<trk`, before its first `<trkseg`.
fn track_name(gpx: &str) -> Option<String> {
    let trk = gpx.find("<trk")?;
    let after = &gpx[trk..];
    let seg = after.find("<trkseg").unwrap_or(after.len());
    tag_text(&after[..seg], "name")
}

type GpxPoint = (String, f64, f64, Option<f64>);

/// Each `<trkpt>` with a latitude and longitude: its time, position, and heart
/// rate, and the number of points without a time. A time with an offset
/// becomes a stored UTC instant; any other time is kept as written.
fn gpx_points(gpx: &str) -> (Vec<GpxPoint>, usize) {
    let mut points = Vec::new();
    let mut untimed = 0;
    for chunk in gpx.split("<trkpt").skip(1) {
        let header = chunk.split('>').next().unwrap_or("");
        let (Some(lat), Some(lon)) = (attr(header, "lat"), attr(header, "lon")) else {
            continue;
        };
        let Some(time) = tag_text(chunk, "time").filter(|value| !value.is_empty()) else {
            untimed += 1;
            continue;
        };
        let hr = tag_text(chunk, "hr").and_then(|value| value.parse().ok());
        points.push((normalize_timestamp(&time).unwrap_or(time), lat, lon, hr));
    }
    (points, untimed)
}

fn attr(header: &str, name: &str) -> Option<f64> {
    let marker = format!("{name}=\"");
    let start = header.find(&marker)? + marker.len();
    let end = header[start..].find('"')? + start;
    header[start..end].parse().ok()
}

/// The trimmed text of the first element with this local name, in any namespace.
fn tag_text(chunk: &str, name: &str) -> Option<String> {
    let mut search = 0;
    while search < chunk.len() {
        let start = search + chunk[search..].find('<')?;
        if chunk.as_bytes().get(start + 1) == Some(&b'/') {
            search = start + 1;
            continue;
        }
        let end = start + chunk[start..].find('>')?;
        let tag = &chunk[start + 1..end];
        let local = tag.split_whitespace().next().unwrap_or(tag);
        let local = local.rsplit(':').next().unwrap_or(local);
        if local == name {
            let content_start = end + 1;
            let close = find_closing_tag(&chunk[content_start..], name)?;
            return Some(chunk[content_start..content_start + close].trim().to_string());
        }
        search = end + 1;
    }
    None
}

fn find_closing_tag(chunk: &str, name: &str) -> Option<usize> {
    let mut search = 0;
    while search < chunk.len() {
        let start = search + chunk[search..].find("</")?;
        let end = start + chunk[start..].find('>')?;
        let tag = &chunk[start + 2..end];
        let local = tag.rsplit(':').next().unwrap_or(tag).trim();
        if local == name {
            return Some(start);
        }
        search = end + 1;
    }
    None
}

fn activity_type(activity: &Map<String, Value>) -> Option<String> {
    match activity.get("activityType") {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Object(object)) => {
            object.get("typeKey").or_else(|| object.get("type")).and_then(Value::as_str).map(str::to_string)
        }
        _ => None,
    }
}

fn text_field(activity: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| activity.get(*key).and_then(Value::as_str)).map(str::to_string)
}

/// An ID as key text: a string as written, a number as Rust prints it.
fn value_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

/// One day read from the Garmin personal connection.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonalDay {
    /// `YYYY-MM-DD`, as the person gave it.
    pub date: String,
    /// The activity search response.
    pub activities: Value,
    /// The HRV response for the day, when the request succeeded.
    pub hrv: Option<Value>,
}

/// The Garmin pull's records: activities and the night's HRV average.
pub struct PersonalReader {
    pub day: PersonalDay,
}

impl RouteReader for PersonalReader {
    fn route(&self) -> Route {
        Route {
            provider: "garmin_personal".into(),
            label: "Garmin personal connection".into(),
            vendor_label: "garmin".into(),
            data_route: "garmin_personal_api".into(),
            container: String::new(),
            match_key_method: "garmin_personal/v1".into(),
            policy: FILE_ROUTE_POLICY,
            report: ReportKind::Pull { source: "garmin_personal".into() },
        }
    }

    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()> {
        let source_id = sink.route_source("garmin", "garmin", None);
        let received_at = sink.received_at().to_string();
        let activities = match &self.day.activities {
            Value::Array(items) => items.clone(),
            other => vec![other.clone()],
        };
        for activity in activities {
            let Some(object) = activity.as_object() else {
                continue;
            };
            if object.get("startTimeGMT").is_none() && object.get("start_at").is_none() {
                object.keys().for_each(|key| sink.preserved(key.clone()));
                continue;
            }
            let workout = workout_from_json(&source_id, object, &received_at).map_err(Error::refused)?;
            workout.understood.iter().for_each(|field| sink.understood(field.clone()));
            workout.preserved.iter().for_each(|field| sink.preserved(field.clone()));
            let key = object.get("activityId").map(|value| value.to_string()).unwrap_or_else(|| {
                workout.data.get("start_at").and_then(Value::as_str).unwrap_or("activity").to_string()
            });
            sink.begin_group(key.clone());
            sink.add(keyed_candidate(RecordType::WorkoutSession, workout.data, &key)?)?;
        }
        let Some(Value::Object(hrv)) = &self.day.hrv else {
            return Ok(());
        };
        let Some(last_night) = hrv.get("LastNightAvg").and_then(Value::as_f64) else {
            hrv.keys().for_each(|key| sink.preserved(key.clone()));
            return Ok(());
        };
        let date = &self.day.date;
        let recorded_at = format!("{date}T00:00:00.000Z");
        let mut data = Map::new();
        data.insert("source_id".into(), Value::String(source_id));
        data.insert("received_at".into(), Value::String(received_at));
        data.insert("received_at_basis".into(), Value::String("import_time".into()));
        data.insert("summary_type".into(), Value::String("LastNightAvg".into()));
        data.insert("recorded_at".into(), Value::String(recorded_at.clone()));
        data.insert("input_timestamp".into(), timestamp_receipt(&recorded_at));
        data.insert(
            "value".into(),
            serde_json::json!({
                "standard_value": last_night,
                "standard_unit": "ms",
                "input_value": last_night,
                "input_unit": "ms"
            }),
        );
        let mut supplied = Map::new();
        for (key, value) in hrv {
            if key != "LastNightAvg" {
                supplied.insert(key.clone(), value.clone());
                sink.preserved(key.clone());
            }
        }
        if !supplied.is_empty() {
            data.insert("supplied_fields".into(), Value::Object(supplied));
        }
        data.insert("field_basis".into(), provider_basis(false));
        sink.understood("LastNightAvg");
        let key = format!("LastNightAvg:{date}");
        sink.begin_group(key.clone());
        sink.add(keyed_candidate(RecordType::ProviderSummary, data, &key)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIVED: &str = "2026-08-09T16:00:00.000Z";

    #[test]
    fn gpx_uses_the_track_name_and_real_sample_times() {
        let gpx = r#"<?xml version="1.0"?>
<gpx><trk><name>Park loop</name><trkseg>
<trkpt lat="40.0" lon="-105.0"><time>2026-08-09T15:00:00Z</time><extensions><hr>100</hr></extensions></trkpt>
<trkpt lat="40.001" lon="-105.001"><time>2026-08-09T15:00:05Z</time><extensions><hr>130</hr></extensions></trkpt>
<trkpt lat="40.002" lon="-105.002"><time>2026-08-09T15:00:10Z</time><extensions><hr>160</hr></extensions></trkpt>
</trkseg></trk></gpx>"#;
        let workout = workout_from_gpx("preview", "file.gpx", gpx, RECEIVED).unwrap();
        assert_eq!(workout.untimed, 0);
        assert_eq!(workout.data["name"], "Park loop");
        assert!(workout.understood.iter().any(|field| field == "heart_rate_samples"));
        let offsets: Vec<i64> = workout.data["heart_rate_samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| sample["offset_ms"].as_i64().unwrap())
            .collect();
        assert_eq!(offsets, [0, 5000, 10000]);
    }

    #[test]
    fn namespaced_heart_rate_is_kept_and_an_untimed_point_is_skipped() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/exports/garmin/namespaced-hr.gpx");
        let gpx = std::fs::read_to_string(path).unwrap();
        let workout = workout_from_gpx("preview", "file.gpx", &gpx, RECEIVED).unwrap();
        assert_eq!(workout.untimed, 1);
        let route = workout.data["route"].as_array().unwrap();
        assert!(route.iter().all(|point| point["recorded_at"].as_str().unwrap().starts_with("2026-08-09")));
        let samples = workout.data["heart_rate_samples"].as_array().unwrap();
        let offsets: Vec<i64> = samples.iter().map(|sample| sample["offset_ms"].as_i64().unwrap()).collect();
        let bpm: Vec<f64> = samples.iter().map(|sample| sample["bpm"].as_f64().unwrap()).collect();
        assert_eq!(offsets, [0, 10000]);
        assert_eq!(bpm, [100.0, 160.0]);
        assert_eq!(workout.data["field_basis"], serde_json::json!({ "value": "provider" }));
    }

    #[test]
    fn a_time_without_an_offset_is_kept_as_written() {
        // The previously observed case: the time is not read in the
        // computer's timezone. The workout check later refuses it.
        let gpx =
            r#"<gpx><trk><trkseg><trkpt lat="1" lon="2"><time>2026-08-09T15:00:00</time></trkpt></trkseg></trk></gpx>"#;
        let workout = workout_from_gpx("preview", "plain.gpx", gpx, RECEIVED).unwrap();
        assert_eq!(workout.data["start_at"], "2026-08-09T15:00:00");
        assert_eq!(
            super::super::shared::validate_workout(&workout.data).unwrap_err(),
            "2026-08-09T15:00:00 is not a UTC timestamp"
        );
    }

    #[test]
    fn a_track_without_points_is_refused() {
        assert_eq!(workout_from_gpx("p", "a.gpx", "<gpx></gpx>", RECEIVED).err().unwrap(), "a.gpx has no track points");
        let untimed = r#"<gpx><trkpt lat="1" lon="2"></trkpt></gpx>"#;
        assert_eq!(
            workout_from_gpx("p", "a.gpx", untimed, RECEIVED).err().unwrap(),
            "a.gpx has no timed track points; 1 had no time"
        );
    }

    #[test]
    fn a_garmin_peak_is_not_the_athlete_maximum() {
        let activity = serde_json::json!({
            "activityId": "1001",
            "activityName": "Park Loop",
            "startTimeGMT": "2026-01-02T15:00:00.000Z",
            "duration": 3,
            "maxHR": 200
        });
        let workout = workout_from_json("preview", activity.as_object().unwrap(), RECEIVED).unwrap();
        assert!(workout.data.get("maximum_heart_rate_bpm").is_none());
        assert!(!workout.understood.iter().any(|field| field == "maximum_heart_rate_bpm"));
    }

    #[test]
    fn every_garmin_workout_states_its_basis() {
        let named = serde_json::json!({
            "activityId": "1002",
            "startTimeGMT": "2026-01-02T15:00:00.000Z",
            "activityType": { "typeKey": "running" }
        });
        let workout = workout_from_json("preview", named.as_object().unwrap(), RECEIVED).unwrap();
        assert_eq!(
            workout.data["field_basis"],
            serde_json::json!({ "value": "provider", "activity_type": "provider_label" })
        );
        let unnamed = serde_json::json!({ "activityId": "1003", "startTimeGMT": "2026-01-02T15:00:00.000Z" });
        let workout = workout_from_json("preview", unnamed.as_object().unwrap(), RECEIVED).unwrap();
        assert_eq!(workout.data["field_basis"], serde_json::json!({ "value": "provider" }));
    }

    #[test]
    fn an_activity_needs_a_start_with_an_offset() {
        let missing = serde_json::json!({ "activityId": 1 });
        assert_eq!(
            workout_from_json("p", missing.as_object().unwrap(), RECEIVED).err().unwrap(),
            "activity is missing startTimeGMT"
        );
        let local = serde_json::json!({ "startTimeGMT": "2026-01-02 15:00:00" });
        assert_eq!(
            workout_from_json("p", local.as_object().unwrap(), RECEIVED).err().unwrap(),
            "could not read time 2026-01-02 15:00:00"
        );
    }
}
