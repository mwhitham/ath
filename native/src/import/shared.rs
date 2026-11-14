//! What the keyed routes share: Garmin and Strava files, the Garmin pull, and
//! mapped imports.
//!
//! In 2.1.0 these routes ran in the Rust helper and wrote through its store.
//! Their records were built as JSON objects and checked by the store's own
//! rules before they were written. The native readers build the same objects,
//! apply the same checks with the same messages, and then decode each object
//! into a typed record. A check that fails stops the preview, so nothing is
//! written.
//!
//! - `validate_workout` is the store's workout check.
//! - `check_supplied_fields` is D191: a supplied field needs a name, and no
//!   name may be `__proto__`, because JavaScript readers drop that key.
//! - `SharedFiles` lists and reads the files of a Garmin or Strava input:
//!   one file, a folder, or a zip archive, within `input_policy`.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::input_policy::{MAX_PROVIDER_FILES, MAX_SHARED_FILE_BYTES};
use crate::json::{Map, Value};
use crate::model::{PathSegment, RecordData, RecordType};

use super::candidate::{Candidate, MatchKey, Section};
use super::input::{self, ZipIndex};
use super::provider::{KeyedPolicy, MatchPolicy};

/// The policy of the Garmin, Strava, and Garmin pull routes. A changed record
/// is laid over the stored one and its revision is a local write, as the
/// 2.1.0 helper's `revise` wrote it.
pub const FILE_ROUTE_POLICY: MatchPolicy = MatchPolicy::Keyed(KeyedPolicy {
    merge: true,
    keep_received_at: false,
    correction_basis: crate::model::BecameKnownAtBasis::LocalWrite,
});

/// The receipt for a time the source wrote as a UTC instant.
pub fn timestamp_receipt(text: &str) -> Value {
    serde_json::json!({
        "timestamp_text": text,
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
    })
}

/// The basis of a value a Garmin or Strava file states as its own (D187).
pub fn provider_basis(activity_type: bool) -> Value {
    if activity_type {
        serde_json::json!({ "value": "provider", "activity_type": "provider_label" })
    } else {
        serde_json::json!({ "value": "provider" })
    }
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for a UTC time, as the 2.1.0 helper wrote it.
pub fn format_datetime(value: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::{Datelike, Timelike};
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        value.year(),
        value.month(),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        value.timestamp_subsec_millis()
    )
}

/// `format_datetime` for milliseconds since 1970. A time chrono cannot hold
/// is 1970, as in 2.1.0.
pub fn format_millis(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(format_datetime)
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".into())
}

/// An RFC 3339 time as a stored UTC instant. Digits past milliseconds are dropped.
pub fn normalize_timestamp(text: &str) -> std::result::Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).map_err(|_| format!("could not read time {text}"))?;
    Ok(format_datetime(parsed.with_timezone(&chrono::Utc)))
}

/// A start time plus a number of seconds, cut to whole milliseconds.
pub fn add_seconds(start: &str, seconds: f64) -> std::result::Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(start).map_err(|error| error.to_string())?;
    let end = parsed + chrono::Duration::milliseconds((seconds * 1000.0) as i64);
    Ok(format_datetime(end.with_timezone(&chrono::Utc)))
}

fn required_str<'a>(data: &'a Map<String, Value>, key: &str) -> std::result::Result<&'a str, String> {
    data.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing {key}"))
}

/// Milliseconds for a stored UTC time. The text must be exactly the stored form.
fn utc_millis(value: &str) -> std::result::Result<i64, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value).map_err(|_| format!("{value} is not a UTC timestamp"))?;
    if value != format_datetime(parsed.with_timezone(&chrono::Utc)) {
        return Err(format!("{value} is not a UTC timestamp with millisecond precision"));
    }
    Ok(parsed.timestamp_millis())
}

/// The 2.1.0 store's check of a workout, with its messages. The first problem
/// found is returned.
pub fn validate_workout(data: &Map<String, Value>) -> std::result::Result<(), String> {
    let start = utc_millis(required_str(data, "start_at")?)?;
    let end = utc_millis(required_str(data, "end_at")?)?;
    if end < start {
        return Err("workout end is before the start".into());
    }
    utc_millis(required_str(data, "received_at")?)?;
    let basis = required_str(data, "received_at_basis")?;
    if !matches!(
        basis,
        "provider_supplied" | "connector_received_time" | "file_modified_time" | "import_time" | "local_write_time"
    ) {
        return Err(format!("received_at_basis {basis} is not a known value"));
    }
    if required_str(data, "source_id")?.is_empty() {
        return Err("missing source_id".into());
    }
    if data.get("source_reported_summaries").and_then(Value::as_array).is_none() {
        return Err("missing source_reported_summaries".into());
    }
    if let Some(name) = data.get("name") {
        if name.as_str().is_none_or(str::is_empty) {
            return Err("workout name is empty".into());
        }
    }
    if let Some(maximum) = data.get("maximum_heart_rate_bpm").and_then(Value::as_f64) {
        if maximum <= 0.0 {
            return Err("maximum heart rate is not a positive number".into());
        }
    }
    if let Some(route) = data.get("route").and_then(Value::as_array) {
        for (index, point) in route.iter().enumerate() {
            let number = index + 1;
            let lat = point
                .get("latitude_deg")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("route point {number} is missing latitude_deg"))?;
            let lon = point
                .get("longitude_deg")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("route point {number} is missing longitude_deg"))?;
            if !(-90.0..=90.0).contains(&lat) {
                return Err(format!("route point {number} latitude is outside -90 to 90"));
            }
            if !(-180.0..=180.0).contains(&lon) {
                return Err(format!("route point {number} longitude is outside -180 to 180"));
            }
            let recorded = point
                .get("recorded_at")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("route point {number} is missing recorded_at"))?;
            utc_millis(recorded)?;
        }
    }
    if let Some(samples) = data.get("heart_rate_samples").and_then(Value::as_array) {
        for (index, sample) in samples.iter().enumerate() {
            let number = index + 1;
            let bpm = sample
                .get("bpm")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("heart rate sample {number} is missing bpm"))?;
            if bpm <= 0.0 {
                return Err(format!("heart rate sample {number} is not a positive number"));
            }
            let offset = sample
                .get("offset_ms")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("heart rate sample {number} is missing offset_ms"))?;
            if offset < 0 {
                return Err(format!("heart rate sample {number} has a negative offset"));
            }
        }
    }
    Ok(())
}

/// D191: every supplied field, on the record and on each route point, needs a
/// name that every reader keeps.
pub fn check_supplied_fields(record_type: &str, data: &Map<String, Value>) -> std::result::Result<(), String> {
    let label = match data.get("provider_record_id").and_then(Value::as_str) {
        Some(id) => format!("{record_type} {id}"),
        None => record_type.to_string(),
    };
    let mut objects: Vec<&Value> = data.get("supplied_fields").into_iter().collect();
    for point in data.get("route").and_then(Value::as_array).into_iter().flatten() {
        objects.extend(point.get("supplied_fields"));
    }
    for fields in objects {
        let Some(fields) = fields.as_object() else {
            return Err(format!("{label}: supplied_fields is not an object"));
        };
        if fields.contains_key("") {
            return Err(format!(
                "{label}: a supplied field has an empty name, and the format needs a name for every field"
            ));
        }
        if fields.iter().any(|(key, child)| key == "__proto__" || contains_proto_key(child)) {
            return Err(format!("{label}: a supplied field is named __proto__, which JavaScript readers drop"));
        }
    }
    Ok(())
}

fn contains_proto_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| key == "__proto__" || contains_proto_key(child)),
        Value::Array(items) => items.iter().any(contains_proto_key),
        _ => false,
    }
}

/// A keyed route's record: the 2.1.0 store's checks, then the record model.
pub fn keyed_candidate(record_type: RecordType, data: Map<String, Value>, key: &str) -> Result<Candidate> {
    if record_type == RecordType::WorkoutSession {
        validate_workout(&data).map_err(Error::refused)?;
    }
    check_supplied_fields(record_type.as_str(), &data).map_err(Error::refused)?;
    let data = RecordData::decode(record_type, &Value::Object(data))
        .map_err(|error| error.under(&[PathSegment::Key("data".into())]))?;
    Ok(Candidate::new(data, MatchKey::literal(key), Section::Record))
}

/// The files of a Garmin or Strava input: one file, a folder, or a zip archive.
///
/// Names are relative to the input, with `/` separators: a folder's files in
/// byte order, depth first, and an archive's entries in archive order. A
/// single file is named by its own name. Files are read one at a time.
pub struct SharedFiles {
    kind: FilesKind,
    names: Vec<String>,
}

enum FilesKind {
    One(PathBuf),
    Folder(PathBuf),
    Zip(ZipIndex),
}

impl SharedFiles {
    pub fn open(path: &Path) -> Result<SharedFiles> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::refused(format!("refusing provider input through a symbolic link: {}", path.display())));
        }
        if metadata.is_dir() {
            return Ok(SharedFiles { names: input::walk_files(path)?, kind: FilesKind::Folder(path.to_path_buf()) });
        }
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        if name.to_ascii_lowercase().ends_with(".zip") {
            let zip = input::open_zip(path)?;
            let names: Vec<String> =
                zip.entries.iter().filter(|entry| !entry.name.ends_with('/')).map(|entry| entry.name.clone()).collect();
            if names.len() > MAX_PROVIDER_FILES {
                return Err(Error::refused(format!("provider export has more than {MAX_PROVIDER_FILES} files")));
            }
            return Ok(SharedFiles { names, kind: FilesKind::Zip(zip) });
        }
        Ok(SharedFiles { names: vec![name], kind: FilesKind::One(path.to_path_buf()) })
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The bytes of one listed file, refused over `MAX_SHARED_FILE_BYTES`.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let too_large = || Error::refused(format!("provider file exceeds {MAX_SHARED_FILE_BYTES} bytes: {name}"));
        let read_file = |path: &Path| -> Result<Vec<u8>> {
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(Error::refused(format!(
                    "refusing provider input that is not a regular file: {}",
                    path.display()
                )));
            }
            if metadata.len() > MAX_SHARED_FILE_BYTES {
                return Err(too_large());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(path)?.take(MAX_SHARED_FILE_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_SHARED_FILE_BYTES {
                return Err(too_large());
            }
            Ok(bytes)
        };
        match &self.kind {
            FilesKind::One(path) => read_file(path),
            FilesKind::Folder(root) => read_file(&root.join(name)),
            FilesKind::Zip(zip) => {
                if zip.entry(name)?.size > MAX_SHARED_FILE_BYTES {
                    return Err(too_large());
                }
                zip.read_entry(name, |reader| {
                    let mut bytes = Vec::new();
                    reader.read_to_end(&mut bytes)?;
                    Ok(bytes)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn a_supplied_field_needs_a_name_that_every_reader_keeps() {
        let empty = object(serde_json::json!({ "provider_record_id": "n-1", "supplied_fields": { "": 1 } }));
        assert_eq!(
            check_supplied_fields("sleep_session", &empty).unwrap_err(),
            "sleep_session n-1: a supplied field has an empty name, and the format needs a name for every field"
        );
        let nested = object(serde_json::json!({ "supplied_fields": { "extra": [{ "__proto__": 1 }] } }));
        assert_eq!(
            check_supplied_fields("source", &nested).unwrap_err(),
            "source: a supplied field is named __proto__, which JavaScript readers drop"
        );
        let route = object(serde_json::json!({ "route": [{ "supplied_fields": { "": 1 } }] }));
        assert!(check_supplied_fields("workout_session", &route).is_err());
        let text = object(serde_json::json!({ "supplied_fields": "not an object" }));
        assert_eq!(
            check_supplied_fields("sleep_session", &text).unwrap_err(),
            "sleep_session: supplied_fields is not an object"
        );
        let kept = object(serde_json::json!({ "supplied_fields": { " ": 1, "proto": 2, "extra": { "": 3 } } }));
        check_supplied_fields("sleep_session", &kept).unwrap();
    }

    #[test]
    fn a_latitude_past_90_is_refused() {
        let data = object(serde_json::json!({
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
        }));
        assert_eq!(validate_workout(&data).unwrap_err(), "route point 1 latitude is outside -90 to 90");
    }

    #[test]
    fn a_time_without_an_offset_is_not_a_stored_instant() {
        assert_eq!(utc_millis("2026-08-09T15:00:00").unwrap_err(), "2026-08-09T15:00:00 is not a UTC timestamp");
        assert_eq!(
            utc_millis("2026-08-09T15:00:00Z").unwrap_err(),
            "2026-08-09T15:00:00Z is not a UTC timestamp with millisecond precision"
        );
        assert_eq!(normalize_timestamp("2026-08-09T10:00:00.1239-05:00").unwrap(), "2026-08-09T15:00:00.123Z");
        assert_eq!(add_seconds("2026-08-09T15:00:00.000Z", 1.2345).unwrap(), "2026-08-09T15:00:01.234Z");
    }
}
