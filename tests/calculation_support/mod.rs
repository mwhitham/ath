//! A small database for calculation tests and skill examples.
//!
//! Records are written in a short form: the fields that matter to a
//! calculation, with an `id` the test chooses. The helper fills in the
//! envelope, the source, timestamps, and receipts, so a test or a skill's
//! worked example states only what the calculation reads.
//!
//! Short forms by `record_type`:
//!
//! - `point_measurement`, `measurement_sample`: `quantity`, `at`, `value`,
//!   optional `basis` (`field_basis.value`) and `source`.
//! - `sleep_session`, `workout_session`: `start`, `end`, and any other data
//!   fields as written, such as `asleep_seconds` or `heart_rate_samples`.
//! - `athlete_report`: `report_type`, `at`, optional `workout` (the `id` of a
//!   workout), and any other data fields.
//! - `athlete_profile`: data fields as written.
//! - `benchmark_result`: `benchmark` (a key), `at`, `score`, optional
//!   `segments` and `extensions`.
//!
//! Any record may name a second `source` (`"source": "b"`); it gets its own
//! wearable source.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use athleticstandard::calculate;
use athleticstandard::json::{self, Value};
use athleticstandard::model::{point_measurement_unit, series_quantity_unit, RecordActor, RecordData, RecordType};
use athleticstandard::store::{Changes, CreateOptions, NewRecord, Store};
use serde_json::json;

/// A JSON value with every number as a double, for comparison.
pub fn canon(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(number) => {
            serde_json::Number::from_f64(number.as_f64().unwrap()).map(serde_json::Value::Number).unwrap()
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(items.iter().map(canon).collect()),
        serde_json::Value::Object(map) => {
            serde_json::Value::Object(map.iter().map(|(key, item)| (key.clone(), canon(item))).collect())
        }
        other => other.clone(),
    }
}

pub struct Lab {
    pub directory: PathBuf,
    pub store: Store,
    ids: BTreeMap<String, String>,
    sources: BTreeMap<String, String>,
    next: u64,
}

fn receipt(at: &str) -> serde_json::Value {
    json!({
        "timestamp_text": at,
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant"
    })
}

fn tool() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "tool", "tool": "test", "tool_version": "1.0.0" })).unwrap()
}

fn athlete() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "athlete" })).unwrap()
}

/// Copy every field except the short-form keys.
fn rest(record: &serde_json::Value, skip: &[&str]) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for (key, value) in record.as_object().unwrap() {
        if !skip.contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
    out
}

impl Lab {
    pub fn new() -> Lab {
        let directory = athleticstandard::files::make_temporary_directory("ath-calculation-lab-").unwrap();
        let store = Store::create(directory.join("athlete.ath.sqlite"), CreateOptions::default()).unwrap();
        Lab { directory, store, ids: BTreeMap::new(), sources: BTreeMap::new(), next: 0x100 }
    }

    /// The record ID for a short `id`, made the first time it is used.
    pub fn id(&mut self, short: &str) -> String {
        if let Some(id) = self.ids.get(short) {
            return id.clone();
        }
        self.next += 1;
        let id = format!("01890f3e-e7d3-7cc0-88c8-{:012x}", self.next);
        self.ids.insert(short.to_string(), id.clone());
        id
    }

    /// The record ID of a named wearable source, made when first used.
    pub fn source_id(&mut self, name: &str) -> String {
        self.source(name)
    }

    fn source(&mut self, name: &str) -> String {
        if let Some(id) = self.sources.get(name) {
            return id.clone();
        }
        let id = self.id(&format!("source:{name}"));
        let data = json!({ "source_type": "wearable", "data_writer": format!("Device {name}"), "data_route": "test" });
        self.create(RecordType::Source, &id, data, None, tool());
        self.sources.insert(name.to_string(), id.clone());
        id
    }

    fn create(
        &mut self,
        record_type: RecordType,
        id: &str,
        data: serde_json::Value,
        extensions: Option<serde_json::Value>,
        actor: RecordActor,
    ) {
        let value: Value = json::parse(&data.to_string()).unwrap();
        let data =
            RecordData::decode(record_type, &value).unwrap_or_else(|error| panic!("{record_type:?} {id}: {error:?}"));
        let extensions = extensions.map(|value| match json::parse(&value.to_string()).unwrap() {
            Value::Object(map) => map,
            _ => panic!("extensions must be an object"),
        });
        let record = NewRecord {
            record_id: Some(id.to_string()),
            created_at: Some("2026-01-01T00:00:00.000Z".to_string()),
            extensions,
            ..NewRecord::new(data, actor)
        };
        self.store.apply(Changes { creates: vec![record], ..Changes::default() }).unwrap();
    }

    /// Write short-form records in order.
    pub fn write(&mut self, records: &[serde_json::Value]) {
        for record in records {
            self.write_one(record);
        }
    }

    pub fn write_one(&mut self, record: &serde_json::Value) {
        let kind = record["record_type"].as_str().unwrap();
        let id = self.id(record["id"].as_str().unwrap());
        let source_name = record.get("source").and_then(|value| value.as_str()).unwrap_or("a").to_string();
        let at = |key: &str| record.get(key).and_then(|value| value.as_str()).map(str::to_string);
        match kind {
            "point_measurement" | "measurement_sample" => {
                let source = self.source(&source_name);
                let quantity = record["quantity"].as_str().unwrap();
                let unit = point_measurement_unit(quantity).or_else(|| series_quantity_unit(quantity)).unwrap();
                let when = at("at").unwrap();
                let value = &record["value"];
                let mut data = json!({
                    "source_id": source,
                    "received_at": when,
                    "received_at_basis": "import_time",
                    "quantity": quantity,
                    "recorded_at": when,
                    "input_timestamp": receipt(&when),
                    "value": { "standard_value": value, "standard_unit": unit, "input_value": value, "input_unit": unit }
                });
                for (key, extra) in rest(record, &["id", "record_type", "quantity", "at", "value", "basis", "source"]) {
                    data[key] = extra;
                }
                if let Some(basis) = record.get("basis") {
                    data["field_basis"] = json!({ "value": basis });
                }
                let record_type = if kind == "point_measurement" {
                    RecordType::PointMeasurement
                } else {
                    RecordType::MeasurementSample
                };
                self.create(record_type, &id, data, None, tool());
            }
            "sleep_session" | "workout_session" => {
                let source = self.source(&source_name);
                let (start, end) = (at("start").unwrap(), at("end").unwrap());
                let mut data = rest(record, &["id", "record_type", "start", "end", "source"]);
                data.insert("source_id".into(), json!(source));
                data.insert("received_at".into(), json!(end));
                data.insert("received_at_basis".into(), json!("import_time"));
                data.insert("start_at".into(), json!(start));
                data.insert("end_at".into(), json!(end));
                data.insert("input_start_timestamp".into(), receipt(&start));
                data.insert("input_end_timestamp".into(), receipt(&end));
                data.entry("source_reported_summaries").or_insert(json!([]));
                let record_type =
                    if kind == "sleep_session" { RecordType::SleepSession } else { RecordType::WorkoutSession };
                self.create(record_type, &id, serde_json::Value::Object(data), None, tool());
            }
            "athlete_report" => {
                let when = at("at").unwrap();
                let mut data = rest(record, &["id", "record_type", "at", "workout"]);
                data.insert("reported_at".into(), json!(when));
                data.insert("input_reported_at".into(), receipt(&when));
                data.entry("reported_via").or_insert(json!("text"));
                if let Some(workout) = at("workout") {
                    data.insert("workout_session_id".into(), json!(self.id(&workout)));
                }
                self.create(RecordType::AthleteReport, &id, serde_json::Value::Object(data), None, athlete());
            }
            "athlete_profile" => {
                let data = rest(record, &["id", "record_type"]);
                self.create(RecordType::AthleteProfile, &id, serde_json::Value::Object(data), None, athlete());
            }
            "benchmark_result" => {
                let source = self.source(&source_name);
                let key = record["benchmark"].as_str().unwrap();
                let definition = self.id(&format!("benchmark:{key}"));
                if let std::collections::btree_map::Entry::Vacant(entry) = self.ids.entry(format!("written:{key}")) {
                    entry.insert(definition.clone());
                    let data = json!({
                        "authorship": {
                            "author": { "author_type": "athlete" },
                            "authored_at": "2026-01-01T00:00:00.000Z",
                            "evidence": [],
                            "content_version": 1,
                            "status": "active"
                        },
                        "benchmark_key": key,
                        "benchmark_type": record.get("benchmark_type").cloned().unwrap_or(json!("custom")),
                        "score_type": "time",
                        "definition": key
                    });
                    self.create(RecordType::BenchmarkDefinition, &definition, data, None, athlete());
                }
                let when = at("at").unwrap();
                let mut data =
                    rest(record, &["id", "record_type", "at", "benchmark", "benchmark_type", "extensions", "source"]);
                data.insert("source_id".into(), json!(source));
                data.insert("received_at".into(), json!(when));
                data.insert("received_at_basis".into(), json!("import_time"));
                data.insert("benchmark_definition_id".into(), json!(definition));
                data.insert("recorded_at".into(), json!(when));
                data.insert("input_timestamp".into(), receipt(&when));
                let extensions = record.get("extensions").cloned();
                self.create(RecordType::BenchmarkResult, &id, serde_json::Value::Object(data), extensions, tool());
            }
            other => panic!("the lab does not write {other}"),
        }
    }

    /// Run calculation file text on a short-form record. The text is
    /// installed as `.agents/skills/<name>/SKILL.md` in a new folder.
    pub fn apply_text(&mut self, text: &str, short: &str) -> serde_json::Value {
        let working = self.directory.join(format!("work-{}", self.next));
        self.next += 1;
        let folder = working.join(".agents/skills/lab-calculation");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("SKILL.md"), text).unwrap();
        let id = self.id(short);
        self.run(&working, "lab-calculation", &id)
    }

    /// Run a shipped calculation skill on a short-form record.
    pub fn apply_shipped(&mut self, name: &str, short: &str) -> serde_json::Value {
        let working = self.directory.join("empty-work");
        std::fs::create_dir_all(&working).unwrap();
        let id = self.id(short);
        self.run(&working, name, &id)
    }

    fn run(&self, working: &Path, name: &str, id: &str) -> serde_json::Value {
        match calculate::apply(&self.store, name, id, working) {
            Ok(result) => serde_json::from_str(&json::encode(&result)).unwrap(),
            Err(error) => json!({ "error": error.to_string() }),
        }
    }

    /// Replace every record ID the lab made with its short name; a source
    /// becomes its name, such as `a`.
    pub fn shorten(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(text) => {
                match self.ids.iter().find(|(short, id)| *id == text && !short.starts_with("written:")) {
                    Some((short, _)) => json!(short.strip_prefix("source:").unwrap_or(short)),
                    None => value.clone(),
                }
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(|item| self.shorten(item)).collect())
            }
            serde_json::Value::Object(map) => {
                serde_json::Value::Object(map.iter().map(|(key, item)| (key.clone(), self.shorten(item))).collect())
            }
            other => other.clone(),
        }
    }

    /// Run a range calculation from a short request: `operation`,
    /// `quantity`, and optional `record_kind`, `source` and `compare_source`
    /// (names), `other_quantity`, `session` (a short ID), `from`, `to`,
    /// `time_zone`, `group`, and `period_type`.
    pub fn calculate(&mut self, request: &serde_json::Value) -> serde_json::Value {
        use athleticstandard::calculate::{CalculateOptions, Group, Operation, RecordKind};
        let text = |key: &str| request.get(key).and_then(|value| value.as_str()).map(str::to_string);
        let mut options = CalculateOptions::new(
            Operation::parse(&text("operation").unwrap()).expect("a range operation"),
            text("quantity").unwrap(),
        );
        if let Some(kind) = text("record_kind") {
            options.record_kind = RecordKind::parse(&kind).unwrap();
        }
        options.source_id = text("source").map(|name| self.source(&name));
        options.compare_source_id = text("compare_source").map(|name| self.source(&name));
        options.other_quantity = text("other_quantity");
        options.workout_session_id = text("session").map(|short| self.id(&short));
        options.from = text("from");
        options.to = text("to");
        options.time_zone = text("time_zone");
        if let Some(group) = text("group") {
            options.group = Group::parse(&group).unwrap();
        }
        options.provider_period_type =
            text("period_type").map(|kind| athleticstandard::model::ProviderReportingPeriodType::parse(&kind).unwrap());
        match calculate::calculate_range(&self.store, &options) {
            Ok(receipt) => serde_json::from_str(&json::encode(&receipt)).unwrap(),
            Err(error) => json!({ "error": error.to_string() }),
        }
    }

    /// A one-line calculation block with settings.
    pub fn block(operation: &str, settings: &[(&str, &str)]) -> String {
        let mut text = format!("```ath-calculation\nname: lab-calculation\nuse: {operation}\n");
        for (key, value) in settings {
            text.push_str(&format!("{key}: {value}\n"));
        }
        text.push_str("```\n");
        text
    }
}

impl Drop for Lab {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
