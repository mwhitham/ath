//! Read and write the same SQLite file the command-line tool uses.

use std::path::Path;

use chrono::{Datelike, Timelike};
use rusqlite::{params, Connection};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::SOFTWARE_VERSION;

pub struct Store {
    conn: Connection,
}

#[derive(Debug)]
pub struct StoredRecord {
    pub record_id: String,
    pub record_type: String,
    pub revision: i64,
    pub data: Value,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|error| error.to_string())?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
            .map_err(|error| error.to_string())?;
        let version: String = conn
            .query_row(
                "SELECT athleticstandard_version FROM database_metadata WHERE singleton_id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("not an Athletic Standard database: {error}"))?;
        if version != SOFTWARE_VERSION {
            return Err(format!(
                "database version is {version}; run ath migrate before using ath-core"
            ));
        }
        Ok(Self { conn })
    }

    pub fn show(&self, record_id: &str) -> Result<StoredRecord, String> {
        self.conn
            .query_row(
                "SELECT records.record_id, records.record_type, records.current_revision, record_revisions.record_json
                 FROM records
                 JOIN record_revisions
                   ON record_revisions.record_id = records.record_id
                  AND record_revisions.revision = records.current_revision
                 WHERE records.record_id = ?1 AND records.is_deleted = 0",
                [record_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .map_err(|error| format!("record {record_id} was not found: {error}"))
            .and_then(|(id, record_type, revision, json)| {
                let data = serde_json::from_str(&json).map_err(|error| error.to_string())?;
                Ok(StoredRecord {
                    record_id: id,
                    record_type,
                    revision,
                    data,
                })
            })
    }

    pub fn revise(&mut self, record_id: &str, patch: &Map<String, Value>) -> Result<StoredRecord, String> {
        let current = self.show(record_id)?;
        let mut data = current
            .data
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{record_id} is not a JSON object"))?;
        for (key, value) in patch {
            data.insert(key.clone(), value.clone());
        }
        if current.record_type == "workout_session" {
            validate_workout(&data)?;
        }
        let json = serde_json::to_string(&Value::Object(data.clone())).map_err(|error| error.to_string())?;
        let next = current.revision + 1;
        let now = utc_now();
        let changed_by = format!(
            "{{\"author_type\":\"tool\",\"tool\":\"ath-core\",\"tool_version\":\"{SOFTWARE_VERSION}\"}}"
        );
        let tx = self
            .conn
            .transaction()
            .map_err(|error| error.to_string())?;
        tx.execute_batch("PRAGMA foreign_keys = ON; PRAGMA defer_foreign_keys = ON;")
            .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO record_revisions (
                record_id, revision, record_type, record_json, extensions_json,
                became_known_at, became_known_at_basis, changed_by_json, previous_revision
             ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, 'local_write', ?6, ?7)",
            params![record_id, next, current.record_type, json, now, changed_by, current.revision],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "UPDATE records SET current_revision = ?1 WHERE record_id = ?2",
            params![next, record_id],
        )
        .map_err(|error| error.to_string())?;
        let table = typed_table(&current.record_type)?;
        tx.execute(
            &format!("UPDATE {table} SET data_json = ?1 WHERE record_id = ?2"),
            params![json, record_id],
        )
        .map_err(|error| error.to_string())?;
        if current.record_type == "workout_session" {
            tx.execute(
                "UPDATE workout_sessions
                 SET source_id = ?1, start_at = ?2, end_at = ?3, activity_type = ?4,
                     name = ?5, description = ?6, text_result = ?7,
                     provider_record_id = ?8, received_at = ?9
                 WHERE record_id = ?10",
                params![
                    required_string(&data, "source_id")?,
                    required_string(&data, "start_at")?,
                    required_string(&data, "end_at")?,
                    optional_string(data.get("activity_type")),
                    optional_string(data.get("name")),
                    optional_string(data.get("description")),
                    optional_string(data.get("text_result")),
                    optional_string(data.get("provider_record_id")),
                    required_string(&data, "received_at")?,
                    record_id,
                ],
            )
            .map_err(|error| error.to_string())?;
            if let Some(route) = data.get("route").and_then(Value::as_array) {
                replace_route(&tx, record_id, route)?;
            }
        }
        if current.record_type == "provider_summary" {
            tx.execute(
                "UPDATE provider_summaries
                 SET source_id = ?1, summary_type = ?2, recorded_at = ?3,
                     standard_value = ?4, standard_unit = ?5,
                     provider_record_id = ?6, received_at = ?7
                 WHERE record_id = ?8",
                params![
                    required_string(&data, "source_id")?,
                    required_string(&data, "summary_type")?,
                    optional_string(data.get("recorded_at")),
                    number_at(&data, &["value", "standard_value"])?,
                    string_at(&data, &["value", "standard_unit"])?,
                    optional_string(data.get("provider_record_id")),
                    required_string(&data, "received_at")?,
                    record_id,
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        tx.execute(
            "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
            [],
        )
        .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        self.show(record_id)
    }

    pub fn ensure_source(&mut self, writer: &str, route: &str, vendor: &str) -> Result<String, String> {
        if let Some(existing) = self
            .conn
            .query_row(
                "SELECT record_id FROM sources
                 WHERE data_writer = ?1 AND data_route = ?2 AND COALESCE(sensor_type, '') = ''",
                params![writer, route],
                |row| row.get::<_, String>(0),
            )
            .ok()
        {
            return Ok(existing);
        }
        let mut data = Map::new();
        data.insert("source_type".into(), Value::String("export_file".into()));
        data.insert("data_writer".into(), Value::String(writer.into()));
        data.insert("data_route".into(), Value::String(route.into()));
        data.insert("vendor".into(), Value::String(vendor.into()));
        self.insert_record("source", &data, None, "provider_import")
    }

    pub fn insert_record(
        &mut self,
        record_type: &str,
        data: &Map<String, Value>,
        match_key: Option<(&str, &str)>,
        basis: &str,
    ) -> Result<String, String> {
        if record_type == "workout_session" {
            validate_workout(data)?;
        }
        if let Some((method, key)) = match_key {
            if let Some(existing) = self
                .conn
                .query_row(
                    "SELECT record_id FROM record_import_keys WHERE match_key_method = ?1 AND match_key = ?2",
                    params![method, key],
                    |row| row.get::<_, String>(0),
                )
                .ok()
            {
                let current = self.show(&existing)?;
                if json_equal(&current.data, &Value::Object(data.clone())) {
                    return Ok(existing);
                }
                let patch = data.clone();
                self.revise(&existing, &patch)?;
                return Ok(existing);
            }
        }
        let record_id = Uuid::now_v7().to_string();
        let now = utc_now();
        let json = serde_json::to_string(&Value::Object(data.clone())).map_err(|error| error.to_string())?;
        let changed_by = format!(
            "{{\"author_type\":\"tool\",\"tool\":\"ath-core\",\"tool_version\":\"{SOFTWARE_VERSION}\"}}"
        );
        let tx = self.conn.transaction().map_err(|error| error.to_string())?;
        tx.execute_batch("PRAGMA foreign_keys = ON; PRAGMA defer_foreign_keys = ON;")
            .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO record_revisions (
                record_id, revision, record_type, record_json, extensions_json,
                became_known_at, became_known_at_basis, changed_by_json, previous_revision
             ) VALUES (?1, 1, ?2, ?3, NULL, ?4, ?5, ?6, NULL)",
            params![record_id, record_type, json, now, basis, changed_by],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
             VALUES (?1, ?2, 1, ?3, 0)",
            params![record_id, record_type, now],
        )
        .map_err(|error| error.to_string())?;
        insert_typed(&tx, record_type, &record_id, data, &json)?;
        if let Some((method, key)) = match_key {
            tx.execute(
                "INSERT INTO record_import_keys (record_id, match_key_method, match_key, imported_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![record_id, method, key, now],
            )
            .map_err(|error| error.to_string())?;
        }
        tx.execute(
            "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
            [],
        )
        .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(record_id)
    }

    pub fn same_source_records(&self, source_id: &str, record_type: &str) -> Result<Vec<StoredRecord>, String> {
        let table = typed_table(record_type)?;
        let mut statement = self
            .conn
            .prepare(&format!(
                "SELECT records.record_id
                 FROM {table}
                 JOIN records ON records.record_id = {table}.record_id
                 WHERE {table}.source_id = ?1 AND records.is_deleted = 0
                 ORDER BY records.created_at, records.record_id"
            ))
            .map_err(|error| error.to_string())?;
        let ids = statement
            .query_map([source_id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        ids.into_iter().map(|id| self.show(&id)).collect()
    }

    pub fn point_records(&self, source_id: &str, quantity: &str) -> Result<Vec<StoredRecord>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT records.record_id
                 FROM point_measurements
                 JOIN records ON records.record_id = point_measurements.record_id
                 WHERE point_measurements.source_id = ?1
                   AND point_measurements.quantity = ?2
                   AND records.is_deleted = 0
                 ORDER BY point_measurements.recorded_at, records.record_id",
            )
            .map_err(|error| error.to_string())?;
        let ids = statement
            .query_map(params![source_id, quantity], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        ids.into_iter().map(|id| self.show(&id)).collect()
    }
}

fn insert_typed(
    tx: &rusqlite::Transaction,
    record_type: &str,
    record_id: &str,
    data: &Map<String, Value>,
    json: &str,
) -> Result<(), String> {
    match record_type {
        "source" => tx.execute(
            "INSERT INTO sources (record_id, record_type, source_type, data_writer, data_route, vendor, sensor_type, data_json)
             VALUES (?1, 'source', ?2, ?3, ?4, ?5, NULL, ?6)",
            params![
                record_id,
                required_string(data, "source_type")?,
                required_string(data, "data_writer")?,
                required_string(data, "data_route")?,
                optional_string(data.get("vendor")),
                json,
            ],
        ),
        "workout_session" => {
            tx.execute(
                "INSERT INTO workout_sessions (
                    record_id, record_type, source_id, start_at, end_at, activity_type,
                    name, description, text_result, provider_record_id, received_at, data_json
                 ) VALUES (?1, 'workout_session', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    record_id,
                    required_string(data, "source_id")?,
                    required_string(data, "start_at")?,
                    required_string(data, "end_at")?,
                    optional_string(data.get("activity_type")),
                    optional_string(data.get("name")),
                    optional_string(data.get("description")),
                    optional_string(data.get("text_result")),
                    optional_string(data.get("provider_record_id")),
                    required_string(data, "received_at")?,
                    json,
                ],
            )
            .map_err(|error| error.to_string())?;
            if let Some(route) = data.get("route").and_then(Value::as_array) {
                replace_route(tx, record_id, route)?;
            }
            return Ok(());
        }
        "sleep_session" => tx.execute(
            "INSERT INTO sleep_sessions (
                record_id, record_type, source_id, start_at, end_at, provider_record_id, received_at, data_json
             ) VALUES (?1, 'sleep_session', ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record_id,
                required_string(data, "source_id")?,
                required_string(data, "start_at")?,
                required_string(data, "end_at")?,
                optional_string(data.get("provider_record_id")),
                required_string(data, "received_at")?,
                json,
            ],
        ),
        "provider_summary" => tx.execute(
            "INSERT INTO provider_summaries (
                record_id, record_type, source_id, summary_type, recorded_at,
                standard_value, standard_unit, provider_record_id, received_at, data_json
             ) VALUES (?1, 'provider_summary', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record_id,
                required_string(data, "source_id")?,
                required_string(data, "summary_type")?,
                optional_string(data.get("recorded_at")),
                number_at(data, &["value", "standard_value"])?,
                string_at(data, &["value", "standard_unit"])?,
                optional_string(data.get("provider_record_id")),
                required_string(data, "received_at")?,
                json,
            ],
        ),
        "point_measurement" => tx.execute(
            "INSERT INTO point_measurements (
                record_id, record_type, source_id, quantity, recorded_at,
                standard_value, standard_unit, provider_record_id, received_at, data_json
             ) VALUES (?1, 'point_measurement', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record_id,
                required_string(data, "source_id")?,
                required_string(data, "quantity")?,
                optional_string(data.get("recorded_at")),
                number_at(data, &["value", "standard_value"])?,
                string_at(data, &["value", "standard_unit"])?,
                optional_string(data.get("provider_record_id")),
                required_string(data, "received_at")?,
                json,
            ],
        ),
        "athlete_report" => tx.execute(
            "INSERT INTO athlete_reports (record_id, record_type, report_type, reported_at, data_json)
             VALUES (?1, 'athlete_report', ?2, ?3, ?4)",
            params![
                record_id,
                required_string(data, "report_type")?,
                required_string(data, "reported_at")?,
                json,
            ],
        ),
        other => return Err(format!("ath-core does not insert {other}")),
    }
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn replace_route(tx: &rusqlite::Transaction, record_id: &str, route: &[Value]) -> Result<(), String> {
    tx.execute(
        "DELETE FROM workout_route_points WHERE workout_session_id = ?1",
        [record_id],
    )
    .map_err(|error| error.to_string())?;
    for (index, point) in route.iter().enumerate() {
        let json = serde_json::to_string(point).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO workout_route_points (workout_session_id, point_order, point_json)
             VALUES (?1, ?2, ?3)",
            params![record_id, index as i64, json],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn typed_table(record_type: &str) -> Result<&'static str, String> {
    match record_type {
        "source" => Ok("sources"),
        "workout_session" => Ok("workout_sessions"),
        "sleep_session" => Ok("sleep_sessions"),
        "provider_summary" => Ok("provider_summaries"),
        "point_measurement" => Ok("point_measurements"),
        "athlete_report" => Ok("athlete_reports"),
        other => Err(format!("no typed table for {other}")),
    }
}

pub fn utc_now() -> String {
    let now = chrono::Utc::now();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.timestamp_subsec_millis()
    )
}

pub fn validate_workout(data: &Map<String, Value>) -> Result<(), String> {
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
        if name.as_str().map(str::is_empty).unwrap_or(true) {
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
            let lat = point.get("latitude_deg").and_then(Value::as_f64).ok_or_else(|| {
                format!("route point {} is missing latitude_deg", index + 1)
            })?;
            let lon = point.get("longitude_deg").and_then(Value::as_f64).ok_or_else(|| {
                format!("route point {} is missing longitude_deg", index + 1)
            })?;
            if !(-90.0..=90.0).contains(&lat) {
                return Err(format!("route point {} latitude is outside -90 to 90", index + 1));
            }
            if !(-180.0..=180.0).contains(&lon) {
                return Err(format!("route point {} longitude is outside -180 to 180", index + 1));
            }
            let recorded = point.get("recorded_at").and_then(Value::as_str).ok_or_else(|| {
                format!("route point {} is missing recorded_at", index + 1)
            })?;
            utc_millis(recorded)?;
        }
    }
    if let Some(samples) = data.get("heart_rate_samples").and_then(Value::as_array) {
        for (index, sample) in samples.iter().enumerate() {
            let bpm = sample.get("bpm").and_then(Value::as_f64).ok_or_else(|| {
                format!("heart rate sample {} is missing bpm", index + 1)
            })?;
            if bpm <= 0.0 {
                return Err(format!("heart rate sample {} is not a positive number", index + 1));
            }
            let offset = sample.get("offset_ms").and_then(Value::as_i64).ok_or_else(|| {
                format!("heart rate sample {} is missing offset_ms", index + 1)
            })?;
            if offset < 0 {
                return Err(format!("heart rate sample {} has a negative offset", index + 1));
            }
        }
    }
    Ok(())
}

fn required_str<'a>(data: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    data.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing {key}"))
}

fn utc_millis(value: &str) -> Result<i64, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| format!("{value} is not a UTC timestamp"))?;
    let formatted = crate::time::format_datetime(parsed.with_timezone(&chrono::Utc));
    if value != formatted {
        return Err(format!("{value} is not a UTC timestamp with millisecond precision"));
    }
    Ok(parsed.timestamp_millis())
}

pub fn timestamp_receipt(text: &str) -> Value {
    serde_json::json!({
        "timestamp_text": text,
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
    })
}

fn required_string(data: &Map<String, Value>, key: &str) -> Result<String, String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing {key}"))
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

fn number_at(data: &Map<String, Value>, path: &[&str]) -> Result<f64, String> {
    let mut current = Value::Object(data.clone());
    for key in path {
        current = current
            .get(*key)
            .cloned()
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    current
        .as_f64()
        .ok_or_else(|| format!("{} is not a number", path.join(".")))
}

fn string_at(data: &Map<String, Value>, path: &[&str]) -> Result<String, String> {
    let mut current = Value::Object(data.clone());
    for key in path {
        current = current
            .get(*key)
            .cloned()
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    current
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{} is not text", path.join(".")))
}

fn json_equal(left: &Value, right: &Value) -> bool {
    left == right
}
