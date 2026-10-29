//! Garmin account export and activity files.

use std::fs;
use std::io::Read;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::fit::read_fit;
use crate::store::{timestamp_receipt, utc_now, Store};

pub fn import_garmin(store: &mut Store, path: &Path, write: bool) -> Result<Value, String> {
    let source_id = if write {
        store.ensure_source("garmin", "garmin_account_export", "garmin")?
    } else {
        "preview".into()
    };
    let mut understood = Vec::new();
    let mut preserved = Vec::new();
    let mut left_out = Vec::new();
    let mut record_ids = Vec::new();
    let received_at = utc_now();
    for (name, bytes) in read_files(path)? {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".fit") {
            match read_fit(&bytes) {
                Ok(points) if !points.is_empty() => {
                    let (data, understood_fields, preserved_fields) =
                        workout_from_fit(&source_id, &name, &points, &received_at);
                    understood.extend(understood_fields);
                    preserved.extend(preserved_fields);
                    if write {
                        let id = store.insert_record(
                            "workout_session",
                            &data,
                            Some(("garmin_account/v1", &name)),
                            "provider_import",
                        )?;
                        record_ids.push(id);
                    }
                }
                Ok(_) => left_out.push(format!("{name}: FIT file had no record points")),
                Err(error) => left_out.push(format!("{name}: {error}")),
            }
            continue;
        }
        if lower.ends_with(".gpx") {
            let gpx = String::from_utf8_lossy(&bytes);
            let (data, understood_fields, preserved_fields) =
                workout_from_gpx(&source_id, &name, &gpx, &received_at)?;
            understood.extend(understood_fields);
            preserved.extend(preserved_fields);
            if write {
                let id = store.insert_record(
                    "workout_session",
                    &data,
                    Some(("garmin_account/v1", &name)),
                    "provider_import",
                )?;
                record_ids.push(id);
            }
            continue;
        }
        if lower.ends_with(".json") {
            let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            for activity in activities_in(&value) {
                let (data, understood_fields, preserved_fields) =
                    workout_from_json(&source_id, activity, &received_at)?;
                understood.extend(understood_fields);
                preserved.extend(preserved_fields.clone());
                let key = activity
                    .get("activityId")
                    .or_else(|| activity.get("activity_id"))
                    .map(value_key)
                    .unwrap_or_else(|| name.clone());
                if write {
                    let id = store.insert_record(
                        "workout_session",
                        &data,
                        Some(("garmin_account/v1", &key)),
                        "provider_import",
                    )?;
                    record_ids.push(id);
                }
            }
            continue;
        }
        left_out.push(format!("{name}: not a training or recovery file"));
    }
    understood.sort();
    understood.dedup();
    preserved.sort();
    preserved.dedup();
    Ok(json!({
        "source": "garmin",
        "record_ids": record_ids,
        "understood": understood,
        "preserved": preserved,
        "left_out": left_out,
    }))
}

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

pub fn workout_from_json(
    source_id: &str,
    activity: &Map<String, Value>,
    received_at: &str,
) -> Result<(Map<String, Value>, Vec<String>, Vec<String>), String> {
    let mut understood = Vec::new();
    let mut preserved = Vec::new();
    let mut data = Map::new();
    let start = activity
        .get("startTimeGMT")
        .or_else(|| activity.get("start_at"))
        .and_then(Value::as_str)
        .map(normalize_timestamp)
        .transpose()?
        .ok_or("activity is missing startTimeGMT")?;
    let duration = activity
        .get("duration")
        .or_else(|| activity.get("elapsedDuration"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let end = add_seconds(&start, duration)?;
    data.insert("source_id".into(), json!(source_id));
    data.insert("received_at".into(), json!(received_at));
    data.insert("received_at_basis".into(), json!("import_time"));
    data.insert("start_at".into(), json!(start));
    data.insert("end_at".into(), json!(end));
    data.insert("input_start_timestamp".into(), timestamp_receipt(&start));
    data.insert("input_end_timestamp".into(), timestamp_receipt(&end));
    data.insert("source_reported_summaries".into(), json!([]));
    understood.extend(["start_at".into(), "end_at".into()]);
    if let Some(name) = text_field(activity, &["activityName", "name"]) {
        data.insert("name".into(), json!(name));
        understood.push("name".into());
    }
    if let Some(description) = text_field(activity, &["description"]) {
        data.insert("description".into(), json!(description));
        understood.push("description".into());
    }
    if let Some(activity_type) = activity_type(activity) {
        data.insert("activity_type".into(), json!(activity_type));
        understood.push("activity_type".into());
    }
    if let Some(id) = activity.get("activityId").map(value_key) {
        data.insert("provider_record_id".into(), json!(id));
        understood.push("provider_record_id".into());
    }
    if let Some(max_hr) = activity.get("maxHR").and_then(Value::as_f64) {
        data.insert("maximum_heart_rate_bpm".into(), json!(max_hr));
        understood.push("maximum_heart_rate_bpm".into());
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
    Ok((data, understood, preserved))
}

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

pub fn workout_from_gpx(
    source_id: &str,
    name: &str,
    gpx: &str,
    received_at: &str,
) -> Result<(Map<String, Value>, Vec<String>, Vec<String>), String> {
    let points = gpx_points(gpx);
    if points.is_empty() {
        return Err(format!("{name} has no track points"));
    }
    let start = points[0].0.clone();
    let end = points[points.len() - 1].0.clone();
    let mut data = Map::new();
    data.insert("source_id".into(), json!(source_id));
    data.insert("received_at".into(), json!(received_at));
    data.insert("received_at_basis".into(), json!("import_time"));
    data.insert("start_at".into(), json!(start));
    data.insert("end_at".into(), json!(end));
    data.insert("input_start_timestamp".into(), timestamp_receipt(&start));
    data.insert("input_end_timestamp".into(), timestamp_receipt(&end));
    data.insert("source_reported_summaries".into(), json!([]));
    let track_name = track_name(gpx).unwrap_or_else(|| name.to_string());
    data.insert("name".into(), json!(track_name));
    data.insert("provider_record_id".into(), json!(name));
    let route: Vec<Value> = points
        .iter()
        .map(|(time, lat, lon, _)| {
            json!({"recorded_at": time, "latitude_deg": lat, "longitude_deg": lon})
        })
        .collect();
    data.insert("route".into(), json!(route));
    let start_ms = parse_time_ms(&points[0].0).unwrap_or(0);
    let samples: Vec<Value> = points
        .iter()
        .filter_map(|(time, _, _, hr)| {
            let bpm = (*hr)?;
            let offset = parse_time_ms(time).unwrap_or(start_ms) - start_ms;
            Some(json!({"offset_ms": offset, "bpm": bpm}))
        })
        .collect();
    let mut understood = vec!["name".into(), "route".into(), "start_at".into(), "end_at".into()];
    if !samples.is_empty() {
        data.insert("heart_rate_samples".into(), json!(samples));
        understood.push("heart_rate_samples".into());
    }
    Ok((data, understood, Vec::new()))
}

fn track_name(gpx: &str) -> Option<String> {
    let trk = gpx.find("<trk")?;
    let after = &gpx[trk..];
    let seg = after.find("<trkseg").unwrap_or(after.len());
    tag_text(&after[..seg], "name")
}

fn parse_time_ms(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.timestamp_millis())
}

fn workout_from_fit(
    source_id: &str,
    name: &str,
    points: &[crate::fit::FitPoint],
    received_at: &str,
) -> (Map<String, Value>, Vec<String>, Vec<String>) {
    let start_ms = points[0].recorded_at_ms;
    let end_ms = points[points.len() - 1].recorded_at_ms.max(start_ms);
    let start = crate::time::format_millis(start_ms);
    let end = crate::time::format_millis(end_ms);
    let mut data = Map::new();
    data.insert("source_id".into(), json!(source_id));
    data.insert("received_at".into(), json!(received_at));
    data.insert("received_at_basis".into(), json!("import_time"));
    data.insert("start_at".into(), json!(start));
    data.insert("end_at".into(), json!(end));
    data.insert("input_start_timestamp".into(), timestamp_receipt(&start));
    data.insert("input_end_timestamp".into(), timestamp_receipt(&end));
    data.insert("source_reported_summaries".into(), json!([]));
    data.insert("name".into(), json!(name));
    data.insert("provider_record_id".into(), json!(name));
    let route: Vec<Value> = points
        .iter()
        .filter_map(|point| {
            Some(json!({
                "recorded_at": crate::time::format_millis(point.recorded_at_ms),
                "latitude_deg": point.latitude_deg?,
                "longitude_deg": point.longitude_deg?,
            }))
        })
        .collect();
    if !route.is_empty() {
        data.insert("route".into(), json!(route));
    }
    let samples: Vec<Value> = points
        .iter()
        .filter_map(|point| {
            point.bpm.map(|bpm| json!({"offset_ms": point.offset_ms, "bpm": bpm}))
        })
        .collect();
    if !samples.is_empty() {
        data.insert("heart_rate_samples".into(), json!(samples));
    }
    (data, vec!["name".into(), "route".into(), "heart_rate_samples".into()], Vec::new())
}

fn gpx_points(gpx: &str) -> Vec<(String, f64, f64, Option<f64>)> {
    let mut points = Vec::new();
    for chunk in gpx.split("<trkpt").skip(1) {
        let header = chunk.split('>').next().unwrap_or("");
        let lat = attr(header, "lat");
        let lon = attr(header, "lon");
        let (Some(lat), Some(lon)) = (lat, lon) else {
            continue;
        };
        let Some(time) = tag_text(chunk, "time").filter(|value| !value.is_empty()) else {
            continue;
        };
        let hr = tag_text(chunk, "hr").and_then(|value| value.parse().ok());
        points.push((normalize_timestamp(&time).unwrap_or(time), lat, lon, hr));
    }
    points
}

fn attr(header: &str, name: &str) -> Option<f64> {
    let marker = format!("{name}=\"");
    let start = header.find(&marker)? + marker.len();
    let end = header[start..].find('"')? + start;
    header[start..end].parse().ok()
}

fn tag_text(chunk: &str, name: &str) -> Option<String> {
    let mut search = 0;
    while search < chunk.len() {
        let rest = &chunk[search..];
        let start = search + rest.find('<')?;
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
        let rest = &chunk[search..];
        let start = search + rest.find("</")?;
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
        Some(Value::Object(object)) => object
            .get("typeKey")
            .or_else(|| object.get("type"))
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn text_field(activity: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| activity.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn value_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

pub fn normalize_timestamp(text: &str) -> Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text)
        .map_err(|_| format!("could not read time {text}"))?;
    let utc = parsed.with_timezone(&chrono::Utc);
    Ok(crate::time::format_datetime(utc))
}

pub fn add_seconds(start: &str, seconds: f64) -> Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(start).map_err(|error| error.to_string())?;
    let end = parsed + chrono::Duration::milliseconds((seconds * 1000.0) as i64);
    Ok(crate::time::format_datetime(end.with_timezone(&chrono::Utc)))
}

#[cfg(test)]
mod tests {
    use super::{import_garmin, workout_from_gpx};
    use crate::store::{validate_workout, Store};

    #[test]
    fn gpx_uses_track_name_and_real_sample_times() {
        let gpx = r#"<?xml version="1.0"?>
<gpx><trk><name>Park loop</name><trkseg>
<trkpt lat="40.0" lon="-105.0"><time>2026-08-09T15:00:00Z</time><extensions><hr>100</hr></extensions></trkpt>
<trkpt lat="40.001" lon="-105.001"><time>2026-08-09T15:00:05Z</time><extensions><hr>130</hr></extensions></trkpt>
<trkpt lat="40.002" lon="-105.002"><time>2026-08-09T15:00:10Z</time><extensions><hr>160</hr></extensions></trkpt>
</trkseg></trk></gpx>"#;
        let (data, understood, _) = workout_from_gpx("preview", "file.gpx", gpx, "2026-08-09T16:00:00.000Z").unwrap();
        assert_eq!(data.get("name").and_then(|value| value.as_str()), Some("Park loop"));
        assert!(understood.iter().any(|field| field == "heart_rate_samples"));
        let samples = data.get("heart_rate_samples").and_then(|value| value.as_array()).unwrap();
        let offsets: Vec<i64> = samples.iter().map(|sample| sample.get("offset_ms").and_then(|value| value.as_i64()).unwrap()).collect();
        assert_eq!(offsets, vec![0, 5000, 10000]);
    }

    #[test]
    fn namespaced_heart_rate_is_kept_and_an_untimed_point_is_skipped() {
        let gpx = include_str!("../../../tests/fixtures/exports/garmin/namespaced-hr.gpx");
        let (data, understood, _) = workout_from_gpx("preview", "file.gpx", gpx, "2026-08-09T16:00:00.000Z").unwrap();
        assert!(understood.iter().any(|field| field == "heart_rate_samples"));
        let route = data.get("route").and_then(|value| value.as_array()).unwrap();
        assert!(route.iter().all(|point| {
            point.get("recorded_at").and_then(|value| value.as_str()).unwrap_or("").starts_with("2026-08-09")
        }));
        let samples = data.get("heart_rate_samples").and_then(|value| value.as_array()).unwrap();
        let offsets: Vec<i64> = samples.iter().map(|sample| sample.get("offset_ms").and_then(|value| value.as_i64()).unwrap()).collect();
        let bpm: Vec<f64> = samples.iter().map(|sample| sample.get("bpm").and_then(|value| value.as_f64()).unwrap()).collect();
        assert_eq!(offsets, vec![0, 10000]);
        assert_eq!(bpm, vec![100.0, 160.0]);
    }

    #[test]
    fn a_latitude_past_90_is_refused() {
        let data = serde_json::json!({
            "source_id": "preview",
            "received_at": "2026-08-09T16:00:00.000Z",
            "received_at_basis": "import_time",
            "start_at": "2026-08-09T15:00:00.000Z",
            "end_at": "2026-08-09T15:10:00.000Z",
            "source_reported_summaries": [],
            "route": [{
                "recorded_at": "2026-08-09T15:00:00.000Z",
                "latitude_deg": 179.99999991618097,
                "longitude_deg": 179.99999991618097
            }]
        });
        let error = validate_workout(data.as_object().unwrap()).unwrap_err();
        assert!(error.contains("latitude"), "{error}");
    }

    #[test]
    fn synthetic_fit_import_omits_the_invalid_position() {
        let dir = std::env::temp_dir().join(format!("ath-fit-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&dir).unwrap();
        let database = dir.join("athlete.ath.sqlite");
        let fit = dir.join("synthetic-position.fit");
        std::fs::write(&fit, include_bytes!("../../../tests/fixtures/exports/garmin/synthetic-position.fit")).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        conn.execute_batch(include_str!("../../../spec/sql/migrations/001-initial.sql")).unwrap();
        conn.execute(
            "INSERT INTO database_metadata (singleton_id, application_marker, sql_schema_version, athleticstandard_version, database_id, created_at) VALUES (1, 'ATH1', 3, '2.0.0', '018f0000-0000-7000-8000-000000000099', '2026-08-09T00:00:00.000Z')",
            [],
        ).unwrap();
        conn.execute("INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0)", []).unwrap();
        conn.execute_batch(include_str!("../../../spec/sql/migrations/003-version-2.sql")).unwrap();
        drop(conn);
        let mut store = Store::open(&database).unwrap();
        let result = import_garmin(&mut store, &fit, true).unwrap();
        assert_eq!(result["record_ids"].as_array().unwrap().len(), 1, "{result}");
        let id = result["record_ids"][0].as_str().unwrap();
        let record = store.show(id).unwrap();
        assert_eq!(record.data["start_at"], "2026-08-09T15:00:00.000Z");
        let route = record.data["route"].as_array().unwrap();
        assert_eq!(route.len(), 1);
        assert_eq!(route[0]["latitude_deg"].as_f64(), Some(0.0));
        assert_eq!(route[0]["longitude_deg"].as_f64(), Some(0.0));
        let samples = record.data["heart_rate_samples"].as_array().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0]["bpm"].as_f64(), Some(120.0));
        std::fs::remove_dir_all(&dir).ok();
    }
}

pub fn read_files(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    if path.is_file() {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .to_string();
        if name.to_ascii_lowercase().ends_with(".zip") {
            return read_zip(&fs::read(path).map_err(|error| error.to_string())?);
        }
        return Ok(vec![(name, fs::read(path).map_err(|error| error.to_string())?)]);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            for (child, bytes) in read_files(&entry.path())? {
                files.push((format!("{file_name}/{child}"), bytes));
            }
        } else {
            files.push((file_name, fs::read(entry.path()).map_err(|error| error.to_string())?));
        }
    }
    Ok(files)
}

fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        if file.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| error.to_string())?;
        files.push((file.name().to_string(), bytes));
    }
    Ok(files)
}
