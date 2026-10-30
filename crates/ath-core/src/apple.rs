//! Read an Apple Health `export.xml` into the same signals the TypeScript import produced.
//!
//! Measured readings name a writer. `Health` and `Manual` are hand-entered, so they
//! stay on the manual source. SDNN stays `hrv_sdnn`. RMSSD is computed from the beats
//! under that record and stored as `hrv_rmssd`. The two are never one series.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::{json, Map, Value};

const SLEEP_GAP_MS: i64 = 3 * 60 * 60 * 1000;
const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const BEAT_WINDOW_TOLERANCE_MS: i64 = 5 * 60 * 1000;
const LONGEST_PLACEABLE_WINDOW_MS: i64 = 55 * 60 * 1000;
const MIN_INTERVAL_MS: f64 = 300.0;
const MAX_INTERVAL_MS: f64 = 2000.0;
const MIN_PAIRS: usize = 10;
const MIN_WINDOW_S: f64 = 10.0;
const GAP_TOLERANCE: f64 = 1.5;

const DEVICE_FIELDS: &[&str] = &[
    "name",
    "manufacturer",
    "model",
    "hardware",
    "software",
    "firmware",
    "localIdentifier",
    "UDI",
];

const SLEEP_ASLEEP: &[&str] = &[
    "HKCategoryValueSleepAnalysisAsleep",
    "HKCategoryValueSleepAnalysisAsleepCore",
    "HKCategoryValueSleepAnalysisAsleepDeep",
    "HKCategoryValueSleepAnalysisAsleepREM",
    "HKCategoryValueSleepAnalysisAsleepUnspecified",
];

pub fn import_apple(path: &str, zip_entry: Option<&str>) -> Result<Value, String> {
    if let Some(entry) = zip_entry {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        let xml = archive
            .by_name(entry)
            .map_err(|error| format!("{entry} in {path}: {error}"))?;
        let mut reader = Reader::from_reader(BufReader::new(xml));
        parse_reader(&mut reader)
    } else {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let mut reader = Reader::from_reader(BufReader::new(file));
        parse_reader(&mut reader)
    }
}

pub fn import_apple_str(xml: &str) -> Result<Value, String> {
    let mut reader = Reader::from_str(xml);
    parse_reader(&mut reader)
}

fn parse_reader<R: BufRead>(reader: &mut Reader<R>) -> Result<Value, String> {
    reader.config_mut().trim_text(true);
    let mut state = State::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref());
                let attrs = attr_map(&event);
                state.on_open(&name, attrs);
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref());
                let attrs = attr_map(&event);
                state.on_open(&name, attrs);
                state.on_close(&name);
            }
            Ok(Event::End(event)) => {
                let name = local_name(event.name().as_ref());
                state.on_close(&name);
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Apple Health XML: {error}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(state.finish())
}

#[derive(Default)]
struct State {
    events: Vec<Value>,
    fragments: Vec<SleepFragment>,
    skipped: Map<String, Value>,
    examples: Map<String, Value>,
    pending: Option<Pending>,
    workout: Option<WorkoutBuild>,
}

impl State {
    fn on_open(&mut self, name: &str, attrs: HashMap<String, String>) {
        match name {
            "Record" => {
                self.pending = None;
                self.open_record(attrs);
            }
            "ActivitySummary" => self.add_activity(attrs),
            "Workout" => self.open_workout(attrs),
            "WorkoutEvent" => self.add_lap(&attrs),
            "MetadataEntry" => self.add_metadata(&attrs),
            "InstantaneousBeatsPerMinute" => self.add_beat(&attrs),
            "ClinicalRecord" => self.bump("clinical records (out of scope)", None, 1),
            _ => {}
        }
    }

    fn on_close(&mut self, name: &str) {
        if name == "Record" {
            if let Some(pending) = self.pending.take() {
                self.commit_record(pending);
            }
        }
        if name == "Workout" {
            if let Some(workout) = self.workout.take() {
                self.events.push(workout.into_event());
            }
        }
    }

    fn finish(self) -> Value {
        let sessions = cluster_sleep(&self.fragments);
        let mut root = Map::new();
        root.insert("events".into(), Value::Array(self.events));
        root.insert("sleep_sessions".into(), Value::Array(sessions));
        root.insert("skipped".into(), Value::Object(self.skipped));
        root.insert("skip_examples".into(), Value::Object(self.examples));
        Value::Object(root)
    }

    fn bump(&mut self, reason: &str, example: Option<String>, n: i64) {
        let current = self.skipped.get(reason).and_then(Value::as_i64).unwrap_or(0);
        self.skipped.insert(reason.to_string(), json!(current + n));
        if let Some(example) = example {
            if !self.examples.contains_key(reason) {
                self.examples.insert(reason.to_string(), json!(example));
            }
        }
    }

    fn open_record(&mut self, attrs: HashMap<String, String>) {
        let raw_type = attrs.get("type").cloned().unwrap_or_default();
        let identifier = identifier_of(&raw_type);
        let start_raw = attrs.get("startDate").cloned().unwrap_or_default();
        let end_raw = attrs.get("endDate").cloned().unwrap_or_default();
        let start = parse_apple_date(&start_raw);
        let end = parse_apple_date(&end_raw);

        if identifier == "SleepAnalysis" {
            if start.is_none() || end.is_none() {
                self.bump("sleep records with an unreadable date", None, 1);
                return;
            }
            let start = start.unwrap();
            let end = end.unwrap();
            let fragment = SleepFragment {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                on_day: on_day(&start_raw),
                start: start.clone(),
                end: end.clone(),
                start_ms: epoch_ms(&start).unwrap_or(0),
                end_ms: epoch_ms(&end).unwrap_or(0),
                input_start: start_raw,
                input_end: end_raw,
                value: attrs.get("value").cloned().unwrap_or_default(),
                source_time_zone: None,
            };
            self.events.push(fragment.event());
            self.pending = Some(Pending::Sleep(fragment));
            return;
        }

        if let Some((kind, convert)) = point_mapping(&identifier) {
            if start.is_none() {
                self.bump("measurements with an unreadable date", None, 1);
                return;
            }
            let Some(raw) = attrs.get("value").and_then(|value| parse_f64(value)) else {
                self.bump("measurements with a non-numeric value", None, 1);
                return;
            };
            let unit = attrs.get("unit").map(String::as_str).unwrap_or("");
            let Some(converted) = convert(raw, unit) else {
                self.bump(
                    "measurements in a unit we do not recognize",
                    Some(format!("{identifier} in \"{}\"", attrs.get("unit").map(String::as_str).unwrap_or("(none)"))),
                    1,
                );
                return;
            };
            if !converted.is_finite() || converted <= 0.0 {
                self.bump("measurements outside a plausible range", None, 1);
                return;
            }
            self.pending = Some(Pending::Point(PointBuild {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                on_day: on_day(&start_raw),
                kind,
                value: round2(converted),
                unit: point_unit(kind).to_string(),
                recorded_at: start.unwrap(),
                input_raw: start_raw,
                input_value: raw,
                input_unit: attrs.get("unit").cloned(),
                source_time_zone: None,
            }));
            return;
        }

        if let Some((quantity, convert)) = series_mapping(&identifier) {
            if start.is_none() {
                self.bump("series samples with an unreadable date", None, 1);
                return;
            }
            let Some(raw) = attrs.get("value").and_then(|value| parse_f64(value)) else {
                self.bump("series samples with a non-numeric value", None, 1);
                return;
            };
            let unit = attrs.get("unit").map(String::as_str).unwrap_or("");
            let Some(converted) = convert(raw, unit) else {
                self.bump(
                    "series samples in a unit we do not recognize",
                    Some(format!("{identifier} in \"{}\"", attrs.get("unit").map(String::as_str).unwrap_or("(none)"))),
                    1,
                );
                return;
            };
            let start = start.unwrap();
            let duration_ms = match (&end, epoch_ms(&start), end.as_deref().and_then(epoch_ms)) {
                (Some(_), Some(start_ms), Some(end_ms)) if end_ms > start_ms => end_ms - start_ms,
                _ => 0,
            };
            self.pending = Some(Pending::Series(SeriesBuild {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                on_day: on_day(&start_raw),
                quantity,
                at: start.clone(),
                value: round3(converted),
                duration_ms,
                input_raw: start_raw,
                input_value: raw,
                input_unit: attrs.get("unit").cloned(),
                source_time_zone: None,
                ends_at: end,
                beats: if quantity == "hrv_sdnn" { Some(Vec::new()) } else { None },
            }));
            return;
        }

        if !raw_type.is_empty() {
            let label = if identifier.is_empty() { raw_type } else { identifier };
            self.bump(&format!("unmapped HealthKit type: {label}"), None, 1);
        }
    }

    fn commit_record(&mut self, pending: Pending) {
        match pending {
            Pending::Sleep(fragment) => self.fragments.push(fragment),
            Pending::Point(point) => self.events.push(point.into_event()),
            Pending::Series(series) => self.events.push(series.into_event()),
        }
    }

    fn add_activity(&mut self, attrs: HashMap<String, String>) {
        let date = attrs.get("dateComponents").map(|value| value.trim()).unwrap_or("");
        if date.len() != 10 || !is_calendar_day(date) {
            self.bump(
                "activity summaries with an unusable date",
                Some(attrs.get("dateComponents").cloned().unwrap_or_else(|| "(none)".into())),
                1,
            );
            return;
        }
        let mut summaries = Vec::new();
        let input_timestamp = json!({
            "timestamp_missing_reason": "not_provided_by_source",
            "utc_offset_missing_reason": "not_provided_by_source",
        });
        let periods = json!([{
            "period_type": "apple_activity_summary_day",
            "provider_date": date,
            "timezone_offset_missing_reason": "not_provided_by_source",
        }]);
        let energy_unit = attrs.get("activeEnergyBurnedUnit").cloned();
        let wants_energy = attrs.contains_key("activeEnergyBurned") || attrs.contains_key("activeEnergyBurnedGoal");
        if wants_energy {
            if energy_unit.is_none() {
                let n = i64::from(attrs.contains_key("activeEnergyBurned"))
                    + i64::from(attrs.contains_key("activeEnergyBurnedGoal"));
                self.bump("activity energy summaries without a source unit", None, n);
            } else {
                let unit = energy_unit.unwrap();
                self.push_summary(&attrs, &mut summaries, "activeEnergyBurned", "active_energy_burned", "kcal", Some(unit.clone()), &input_timestamp, &periods, true);
                self.push_summary(&attrs, &mut summaries, "activeEnergyBurnedGoal", "active_energy_goal", "kcal", Some(unit), &input_timestamp, &periods, true);
            }
        }
        self.push_summary(&attrs, &mut summaries, "appleExerciseTime", "exercise_time", "min", None, &input_timestamp, &periods, false);
        self.push_summary(&attrs, &mut summaries, "appleExerciseTimeGoal", "exercise_time_goal", "min", None, &input_timestamp, &periods, false);
        self.push_summary(&attrs, &mut summaries, "appleStandHours", "stand_hours", "h", None, &input_timestamp, &periods, false);
        self.push_summary(&attrs, &mut summaries, "appleStandHoursGoal", "stand_hours_goal", "h", None, &input_timestamp, &periods, false);
        self.events.push(json!({
            "kind": "activity",
            "writer": activity_writer(&attrs),
            "device_raw": device_raw(&attrs),
            "on_day": on_day(attrs.get("startDate").map(String::as_str).unwrap_or("")),
            "summaries": summaries,
        }));
    }

    fn push_summary(
        &mut self,
        attrs: &HashMap<String, String>,
        summaries: &mut Vec<Value>,
        attribute: &str,
        summary_type: &str,
        standard_unit: &str,
        input_unit: Option<String>,
        input_timestamp: &Value,
        periods: &Value,
        energy: bool,
    ) {
        let Some(raw_text) = attrs.get(attribute) else { return };
        let input_value = parse_f64(raw_text);
        let converted = match (input_value, energy, &input_unit) {
            (Some(value), true, Some(unit)) => to_kilocalories(value, unit),
            (Some(value), false, _) => Some(value),
            _ => None,
        };
        if input_value.is_none() || converted.is_none() || converted.is_some_and(|value| !value.is_finite() || value < 0.0) {
            self.bump(
                "activity summary values we could not normalize",
                Some(format!("{attribute}=\"{raw_text}\"")),
                1,
            );
            return;
        }
        let value = round2(converted.unwrap());
        let mut summary = Map::new();
        summary.insert("summary_type".into(), json!(summary_type));
        summary.insert("value".into(), num(value));
        summary.insert("standard_unit".into(), json!(standard_unit));
        summary.insert("input_value".into(), num(input_value.unwrap()));
        if let Some(unit) = input_unit {
            summary.insert("input_unit".into(), json!(unit));
        } else {
            summary.insert("input_unit_missing_reason".into(), json!("not_exposed_by_route"));
        }
        summary.insert("input_timestamp".into(), input_timestamp.clone());
        summary.insert("provider_reporting_periods".into(), periods.clone());
        summaries.push(Value::Object(summary));
    }

    fn open_workout(&mut self, attrs: HashMap<String, String>) {
        let start_raw = attrs.get("startDate").cloned().unwrap_or_default();
        let end_raw = attrs.get("endDate").cloned().unwrap_or_default();
        let start = parse_apple_date(&start_raw);
        let end = parse_apple_date(&end_raw);
        let usable = match (&start, &end) {
            (Some(start), Some(end)) => match (epoch_ms(start), epoch_ms(end)) {
                (Some(start_ms), Some(end_ms)) => end_ms > start_ms,
                _ => false,
            },
            _ => false,
        };
        if !usable {
            let activity = attrs
                .get("workoutActivityType")
                .map(|value| value.trim_start_matches("HKWorkoutActivityType"))
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown");
            self.bump(
                "workouts with an unusable time range",
                Some(format!(
                    "{activity} from \"{}\" to \"{}\"",
                    if start_raw.is_empty() { "(none)".into() } else { start_raw.clone() },
                    if end_raw.is_empty() { "(none)".into() } else { end_raw.clone() },
                )),
                1,
            );
            return;
        }
        let mut aggregates = Map::new();
        let mut input_summaries = Map::new();
        let activity = attrs
            .get("workoutActivityType")
            .map(|value| value.trim_start_matches("HKWorkoutActivityType").to_lowercase())
            .unwrap_or_default();
        if !activity.is_empty() {
            aggregates.insert("activity".into(), json!(activity));
        }
        if let Some(distance) = attrs.get("totalDistance").and_then(|value| parse_f64(value)) {
            if distance > 0.0 {
                let unit = attrs.get("totalDistanceUnit").cloned().unwrap_or_else(|| "m".into());
                let metres = to_metres(distance, &unit).unwrap_or(distance);
                aggregates.insert("distance_m".into(), num(round2(metres)));
                input_summaries.insert("distance_m".into(), json!({ "input_value": distance, "input_unit": unit }));
            }
        }
        if let Some(energy) = attrs.get("totalEnergyBurned").and_then(|value| parse_f64(value)) {
            if energy > 0.0 {
                let unit = attrs.get("totalEnergyBurnedUnit").cloned().unwrap_or_else(|| "kcal".into());
                aggregates.insert("energy_kcal".into(), num(round2(energy)));
                input_summaries.insert("energy_kcal".into(), json!({ "input_value": energy, "input_unit": unit }));
            }
        }
        self.workout = Some(WorkoutBuild {
            writer: writer_name(&attrs),
            device_raw: device_raw(&attrs),
            on_day: on_day(&start_raw),
            start: start.unwrap(),
            end: end.unwrap(),
            input_start: start_raw,
            input_end: end_raw,
            aggregates,
            input_summaries,
            segments: Vec::new(),
            source_time_zone: None,
        });
    }

    fn add_lap(&mut self, attrs: &HashMap<String, String>) {
        let Some(workout) = self.workout.as_mut() else { return };
        if attrs.get("type").map(String::as_str) != Some("HKWorkoutEventTypeLap") {
            return;
        }
        let mut segment = Map::new();
        segment.insert("label".into(), json!(format!("lap {}", workout.segments.len() + 1)));
        if let Some(duration) = attrs.get("duration").and_then(|value| parse_f64(value)) {
            if duration > 0.0 {
                segment.insert("duration_s".into(), num(round2(duration * 60.0)));
            }
        }
        workout.segments.push(Value::Object(segment));
    }

    fn add_metadata(&mut self, attrs: &HashMap<String, String>) {
        if attrs.get("key").map(String::as_str) == Some("HKTimeZone") {
            if let Some(zone) = attrs.get("value").map(|value| value.trim()).filter(|value| !value.is_empty()) {
                let zone = zone.to_string();
                if let Some(pending) = self.pending.as_mut() {
                    pending.set_zone(zone.clone());
                }
                if let Some(workout) = self.workout.as_mut() {
                    workout.source_time_zone = Some(zone);
                }
            }
            return;
        }
        let Some(workout) = self.workout.as_mut() else { return };
        let Some(value) = attrs.get("value").and_then(|value| parse_f64(value)) else { return };
        if value <= 0.0 {
            return;
        }
        let key = attrs.get("key").map(String::as_str).unwrap_or("");
        if key == "HKAverageHeartRate" {
            workout.aggregates.insert("avg_hr_bpm".into(), num(round2(value)));
            workout.input_summaries.insert("avg_hr_bpm".into(), json!({ "input_value": value, "input_unit": "bpm" }));
        }
        if key == "HKMaximumHeartRate" {
            workout.aggregates.insert("max_hr_bpm".into(), num(round2(value)));
            workout.input_summaries.insert("max_hr_bpm".into(), json!({ "input_value": value, "input_unit": "bpm" }));
        }
    }

    fn add_beat(&mut self, attrs: &HashMap<String, String>) {
        let bpm_text = attrs.get("bpm").cloned();
        let time = attrs.get("time").cloned().unwrap_or_default();
        let parsed = bpm_text.as_deref().and_then(parse_f64);
        let placed = self.pending.as_ref().and_then(|pending| match pending {
            Pending::Series(series) if series.beats.is_some() => {
                Some((series.at.clone(), series.ends_at.clone(), parsed))
            }
            _ => None,
        });
        let Some((at, ends_at, bpm)) = placed else { return };
        let Some(bpm) = bpm.filter(|value| *value > 0.0) else {
            self.bump(
                "heartbeat readings with an unusable rate",
                Some(format!("bpm \"{}\"", bpm_text.as_deref().unwrap_or("(none)"))),
                1,
            );
            return;
        };
        let Some(offset_ms) = beat_offset_ms(&at, ends_at.as_deref(), &time) else {
            self.bump(
                "heartbeat readings we could not place in their reading's window",
                Some(format!(
                    "time \"{}\" in a window starting {}",
                    if time.is_empty() { "(none)" } else { time.as_str() },
                    at
                )),
                1,
            );
            return;
        };
        if let Some(Pending::Series(series)) = self.pending.as_mut() {
            if let Some(beats) = series.beats.as_mut() {
                beats.push(Beat { offset_ms, interval_ms: 60000.0 / bpm });
            }
        }
    }
}

enum Pending {
    Point(PointBuild),
    Series(SeriesBuild),
    Sleep(SleepFragment),
}

impl Pending {
    fn set_zone(&mut self, zone: String) {
        match self {
            Pending::Point(point) => point.source_time_zone = Some(zone),
            Pending::Series(series) => series.source_time_zone = Some(zone),
            Pending::Sleep(fragment) => fragment.source_time_zone = Some(zone),
        }
    }
}

struct PointBuild {
    writer: String,
    device_raw: String,
    on_day: String,
    kind: &'static str,
    value: f64,
    unit: String,
    recorded_at: String,
    input_raw: String,
    input_value: f64,
    input_unit: Option<String>,
    source_time_zone: Option<String>,
}

impl PointBuild {
    fn into_event(self) -> Value {
        let mut receipt = Map::new();
        receipt.insert(
            "input_timestamp".into(),
            timestamp_receipt(&self.input_raw, self.source_time_zone.as_deref()),
        );
        receipt.insert("input_value".into(), num(self.input_value));
        if let Some(unit) = self.input_unit {
            receipt.insert("input_unit".into(), json!(unit));
        } else {
            receipt.insert("input_unit_missing_reason".into(), json!("not_provided_by_source"));
        }
        json!({
            "kind": "point",
            "writer": self.writer,
            "device_raw": self.device_raw,
            "on_day": self.on_day,
            "type": self.kind,
            "value": self.value,
            "unit": self.unit,
            "recorded_at": self.recorded_at,
            "receipt": Value::Object(receipt),
        })
    }
}

struct Beat {
    offset_ms: i64,
    interval_ms: f64,
}

struct SeriesBuild {
    writer: String,
    device_raw: String,
    on_day: String,
    quantity: &'static str,
    at: String,
    value: f64,
    duration_ms: i64,
    input_raw: String,
    input_value: f64,
    input_unit: Option<String>,
    source_time_zone: Option<String>,
    ends_at: Option<String>,
    beats: Option<Vec<Beat>>,
}

impl SeriesBuild {
    fn into_event(self) -> Value {
        let mut sample = Map::new();
        sample.insert("at".into(), json!(self.at));
        sample.insert("value".into(), num(self.value));
        if self.duration_ms > 0 {
            sample.insert("durationMs".into(), json!(self.duration_ms));
        }
        sample.insert("inputTimestampText".into(), json!(self.input_raw));
        if let Some(offset) = trailing_offset(self.input_raw.trim()) {
            sample.insert("inputUtcOffsetText".into(), json!(offset));
        }
        sample.insert("inputUtcOffsetMeaning".into(), json!("meaning_not_documented"));
        sample.insert("inputTimestampBasis".into(), json!("source_offset_timestamp"));
        sample.insert("inputValue".into(), num(self.input_value));
        if let Some(unit) = &self.input_unit {
            sample.insert("inputUnit".into(), json!(unit));
        }
        if let Some(zone) = &self.source_time_zone {
            sample.insert("inputSourceTimeZone".into(), json!(zone));
            sample.insert(
                "inputSourceTimeZoneMeaning".into(),
                json!("apple_health_object_creation_time_zone"),
            );
        }
        let mut event = Map::new();
        event.insert("kind".into(), json!("series"));
        event.insert("writer".into(), json!(self.writer));
        event.insert("device_raw".into(), json!(self.device_raw));
        event.insert("on_day".into(), json!(self.on_day));
        event.insert("quantity".into(), json!(self.quantity));
        event.insert("sample".into(), Value::Object(sample));
        if let Some(beats) = &self.beats {
            if !beats.is_empty() {
                let start_ms = epoch_ms(&self.at).unwrap_or(0);
                let beat_samples = beats
                    .iter()
                    .map(|beat| {
                        json!({
                            "at": at_offset_of(&self.at, start_ms as f64 + beat.offset_ms as f64),
                            "value": round1(beat.interval_ms),
                        })
                    })
                    .collect::<Vec<_>>();
                event.insert("beat_samples".into(), Value::Array(beat_samples));
                if let Some(derived) = rmssd_from_beats(beats) {
                    let mut signal = Map::new();
                    signal.insert("type".into(), json!("hrv_rmssd"));
                    signal.insert("value".into(), num(derived.rmssd_ms));
                    signal.insert("unit".into(), json!("ms"));
                    signal.insert("recorded_at".into(), json!(self.at));
                    let mut body = Map::new();
                    body.insert("from".into(), json!("hrv_beats"));
                    body.insert("method".into(), json!("rmssd"));
                    body.insert("window_s".into(), num(derived.window_s));
                    body.insert("n_beats".into(), json!(derived.n_beats));
                    if derived.n_dropped > 0 {
                        body.insert("n_dropped".into(), json!(derived.n_dropped));
                    }
                    signal.insert("derived".into(), Value::Object(body));
                    let receipt = json!({
                        "input_timestamp": timestamp_receipt(&self.input_raw, self.source_time_zone.as_deref()),
                        "input_value_missing_reason": "not_applicable",
                        "input_unit_missing_reason": "not_applicable",
                    });
                    event.insert("derived".into(), json!({ "signal": Value::Object(signal), "receipt": receipt }));
                }
            }
        }
        Value::Object(event)
    }
}

struct WorkoutBuild {
    writer: String,
    device_raw: String,
    on_day: String,
    start: String,
    end: String,
    input_start: String,
    input_end: String,
    aggregates: Map<String, Value>,
    input_summaries: Map<String, Value>,
    segments: Vec<Value>,
    source_time_zone: Option<String>,
}

impl WorkoutBuild {
    fn into_event(self) -> Value {
        let mut event = Map::new();
        event.insert("kind".into(), json!("workout"));
        event.insert("writer".into(), json!(self.writer));
        event.insert("device_raw".into(), json!(self.device_raw));
        event.insert("on_day".into(), json!(self.on_day));
        event.insert("start".into(), json!(self.start));
        event.insert("end".into(), json!(self.end));
        event.insert("aggregates".into(), Value::Object(self.aggregates));
        if !self.segments.is_empty() {
            event.insert("segments".into(), Value::Array(self.segments));
        }
        let mut receipt = Map::new();
        receipt.insert(
            "input_start_timestamp".into(),
            timestamp_receipt(&self.input_start, self.source_time_zone.as_deref()),
        );
        receipt.insert(
            "input_end_timestamp".into(),
            timestamp_receipt(&self.input_end, self.source_time_zone.as_deref()),
        );
        receipt.insert("input_summaries".into(), Value::Object(self.input_summaries));
        event.insert("receipt".into(), Value::Object(receipt));
        Value::Object(event)
    }
}

struct SleepFragment {
    writer: String,
    device_raw: String,
    on_day: String,
    start: String,
    end: String,
    start_ms: i64,
    end_ms: i64,
    input_start: String,
    input_end: String,
    value: String,
    source_time_zone: Option<String>,
}

impl SleepFragment {
    fn event(&self) -> Value {
        json!({
            "kind": "sleep_fragment",
            "writer": self.writer,
            "device_raw": self.device_raw,
            "on_day": self.on_day,
        })
    }
}

struct Rmssd {
    rmssd_ms: f64,
    n_beats: usize,
    n_dropped: usize,
    window_s: f64,
}

fn cluster_sleep(fragments: &[SleepFragment]) -> Vec<Value> {
    let mut groups: Vec<(String, Vec<SleepFragment>)> = Vec::new();
    for fragment in fragments {
        let key = if hand_entered(&fragment.writer) {
            "\u{0}manual".to_string()
        } else {
            fragment.writer.clone()
        };
        if let Some((_, own)) = groups.iter_mut().find(|(name, _)| name == &key) {
            own.push(fragment.clone());
        } else {
            groups.push((key, vec![fragment.clone()]));
        }
    }
    groups
        .into_iter()
        .flat_map(|(_, own)| cluster_nights(own))
        .collect()
}

fn cluster_nights(mut fragments: Vec<SleepFragment>) -> Vec<Value> {
    if fragments.is_empty() {
        return Vec::new();
    }
    fragments.sort_by(|a, b| a.start_ms.cmp(&b.start_ms));
    let mut clusters: Vec<Vec<SleepFragment>> = Vec::new();
    let mut current = vec![fragments[0].clone()];
    let mut cluster_end = fragments[0].end_ms;
    for fragment in fragments.into_iter().skip(1) {
        if fragment.start_ms - cluster_end > SLEEP_GAP_MS {
            clusters.push(current);
            cluster_end = fragment.end_ms;
            current = vec![fragment];
            continue;
        }
        cluster_end = cluster_end.max(fragment.end_ms);
        current.push(fragment);
    }
    clusters.push(current);

    clusters
        .into_iter()
        .filter_map(|cluster| {
            let start_at = cluster.iter().enumerate().min_by_key(|(_, fragment)| fragment.start_ms).map(|(index, _)| index).unwrap_or(0);
            let mut end_at = 0;
            for (index, fragment) in cluster.iter().enumerate().skip(1) {
                if fragment.end_ms > cluster[end_at].end_ms {
                    end_at = index;
                }
            }
            let start = &cluster[start_at];
            let end = &cluster[end_at];
            if end.end_ms <= start.start_ms {
                return None;
            }
            let mut in_bed = 0.0;
            let mut asleep = 0.0;
            let mut deep = 0.0;
            let mut rem = 0.0;
            let mut light = 0.0;
            let mut awake = 0.0;
            let mut interruptions = 0;
            for fragment in &cluster {
                let seconds = (fragment.end_ms - fragment.start_ms) as f64 / 1000.0;
                if fragment.value == "HKCategoryValueSleepAnalysisInBed" {
                    in_bed += seconds;
                } else if fragment.value == "HKCategoryValueSleepAnalysisAwake" {
                    awake += seconds;
                    interruptions += 1;
                } else if SLEEP_ASLEEP.contains(&fragment.value.as_str()) {
                    asleep += seconds;
                    if fragment.value == "HKCategoryValueSleepAnalysisAsleepDeep" {
                        deep += seconds;
                    } else if fragment.value == "HKCategoryValueSleepAnalysisAsleepREM" {
                        rem += seconds;
                    } else if fragment.value == "HKCategoryValueSleepAnalysisAsleepCore" {
                        light += seconds;
                    }
                }
            }
            let mut aggregates = Map::new();
            if asleep > 0.0 {
                aggregates.insert("duration_s".into(), num(js_round(asleep)));
            }
            if in_bed > 0.0 {
                aggregates.insert("time_in_bed_s".into(), num(js_round(in_bed)));
            }
            if deep > 0.0 {
                aggregates.insert("deep_s".into(), num(js_round(deep)));
            }
            if rem > 0.0 {
                aggregates.insert("rem_s".into(), num(js_round(rem)));
            }
            if light > 0.0 {
                aggregates.insert("light_s".into(), num(js_round(light)));
            }
            if awake > 0.0 {
                aggregates.insert("awake_s".into(), num(js_round(awake)));
            }
            if interruptions > 0 {
                aggregates.insert("interruptions".into(), json!(interruptions));
            }
            if in_bed > 0.0 && asleep > 0.0 {
                aggregates.insert(
                    "efficiency_pct".into(),
                    num((js_round((asleep / in_bed) * 1000.0) / 10.0).min(100.0)),
                );
            }
            let device_raw = cluster
                .iter()
                .find(|fragment| parse_apple_device(&fragment.device_raw).is_some())
                .map(|fragment| fragment.device_raw.clone())
                .unwrap_or_default();
            let mut receipt = Map::new();
            receipt.insert(
                "input_start_timestamp".into(),
                timestamp_receipt(&start.input_start, start.source_time_zone.as_deref()),
            );
            receipt.insert(
                "input_end_timestamp".into(),
                timestamp_receipt(&end.input_end, end.source_time_zone.as_deref()),
            );
            Some(json!({
                "writer": start.writer,
                "device_raw": device_raw,
                "start": start.start,
                "end": end.end,
                "aggregates": Value::Object(aggregates),
                "receipt": Value::Object(receipt),
            }))
        })
        .collect()
}

fn rmssd_from_beats(beats: &[Beat]) -> Option<Rmssd> {
    let plausible: Vec<&Beat> = beats
        .iter()
        .filter(|beat| beat.interval_ms >= MIN_INTERVAL_MS && beat.interval_ms <= MAX_INTERVAL_MS)
        .collect();
    let n_dropped = beats.len() - plausible.len();
    if plausible.len() < 2 {
        return None;
    }
    let mut runs: Vec<Vec<&Beat>> = Vec::new();
    let mut run = vec![plausible[0]];
    for index in 1..plausible.len() {
        let previous = plausible[index - 1];
        let current = plausible[index];
        let observed = current.offset_ms - previous.offset_ms;
        let continuous = observed > 0 && (observed as f64) <= current.interval_ms * GAP_TOLERANCE;
        if continuous {
            run.push(current);
        } else {
            runs.push(run);
            run = vec![current];
        }
    }
    runs.push(run);
    let mut sum_squares = 0.0;
    let mut pairs = 0usize;
    for segment in &runs {
        for index in 1..segment.len() {
            let diff = segment[index].interval_ms - segment[index - 1].interval_ms;
            sum_squares += diff * diff;
            pairs += 1;
        }
    }
    if pairs < MIN_PAIRS {
        return None;
    }
    let span_ms = plausible[plausible.len() - 1].offset_ms - plausible[0].offset_ms;
    let window_s = span_ms as f64 / 1000.0;
    if window_s < MIN_WINDOW_S {
        return None;
    }
    Some(Rmssd {
        rmssd_ms: round1((sum_squares / pairs as f64).sqrt()),
        n_beats: plausible.len(),
        n_dropped,
        window_s: round1(window_s),
    })
}

fn hand_entered(writer: &str) -> bool {
    let lower = writer.to_lowercase();
    lower == "health" || lower == "manual"
}

fn writer_name(attrs: &HashMap<String, String>) -> String {
    let name = attrs.get("sourceName").map(|value| value.trim()).unwrap_or("");
    if name.is_empty() { "Unknown".into() } else { name.to_string() }
}

fn activity_writer(attrs: &HashMap<String, String>) -> String {
    let name = attrs.get("sourceName").map(|value| value.trim()).unwrap_or("");
    if name.is_empty() { "Apple Health".into() } else { name.to_string() }
}

fn device_raw(attrs: &HashMap<String, String>) -> String {
    attrs.get("device").cloned().unwrap_or_default()
}

fn on_day(raw: &str) -> String {
    let day: String = raw.chars().take(10).collect();
    if is_calendar_day(&day) { day } else { String::new() }
}

fn is_calendar_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn identifier_of(raw_type: &str) -> String {
    raw_type
        .trim_start_matches("HKQuantityTypeIdentifier")
        .trim_start_matches("HKCategoryTypeIdentifier")
        .trim_start_matches("HKDataType")
        .to_string()
}

fn point_mapping(identifier: &str) -> Option<(&'static str, fn(f64, &str) -> Option<f64>)> {
    Some(match identifier {
        "RestingHeartRate" => ("resting_heart_rate", per_minute as fn(f64, &str) -> Option<f64>),
        "WalkingHeartRateAverage" => ("walking_heart_rate", per_minute),
        "HeartRateRecoveryOneMinute" => ("hr_recovery", per_minute),
        "BodyMass" => ("body_weight", to_kilograms),
        "LeanBodyMass" => ("lean_body_mass", to_kilograms),
        "BodyFatPercentage" => ("body_fat_percentage", to_percent),
        "Height" => ("height", to_centimeters),
        "VO2Max" => ("vo2_max", passthrough),
        "AppleSleepingWristTemperature" => ("wrist_temperature_sleeping", to_celsius),
        "BodyTemperature" => ("body_temperature", to_celsius),
        "BloodPressureSystolic" => ("blood_pressure_systolic", to_millimetres_mercury),
        "BloodPressureDiastolic" => ("blood_pressure_diastolic", to_millimetres_mercury),
        _ => return None,
    })
}

fn point_unit(kind: &str) -> &'static str {
    match kind {
        "resting_heart_rate" | "walking_heart_rate" | "hr_recovery" => "bpm",
        "body_weight" | "lean_body_mass" => "kg",
        "body_fat_percentage" => "%",
        "height" => "cm",
        "vo2_max" => "ml/kg/min",
        "wrist_temperature_sleeping" | "body_temperature" => "°C",
        "blood_pressure_systolic" | "blood_pressure_diastolic" => "mmHg",
        _ => "",
    }
}

fn series_mapping(identifier: &str) -> Option<(&'static str, fn(f64, &str) -> Option<f64>)> {
    Some(match identifier {
        "HeartRate" => ("heart_rate", per_minute as fn(f64, &str) -> Option<f64>),
        "HeartRateVariabilitySDNN" => ("hrv_sdnn", to_milliseconds),
        "RespiratoryRate" => ("respiratory_rate", per_minute),
        "OxygenSaturation" => ("oxygen_saturation", to_percent),
        "StepCount" => ("steps", to_count),
        "ActiveEnergyBurned" => ("active_energy", to_kilocalories),
        "BasalEnergyBurned" => ("basal_energy", to_kilocalories),
        "DistanceWalkingRunning" => ("distance_walking_running", to_metres),
        "DistanceCycling" => ("distance_cycling", to_metres),
        "DistanceSwimming" => ("distance_swimming", to_metres),
        "RunningSpeed" => ("running_speed", to_metres_per_second),
        "RunningPower" => ("running_power", to_watts),
        "RunningStrideLength" => ("running_stride_length", to_metres),
        "RunningVerticalOscillation" => ("running_vertical_oscillation", to_centimeters),
        "RunningGroundContactTime" => ("running_ground_contact_time", to_milliseconds),
        "PhysicalEffort" => ("physical_effort", to_mets),
        "AppleExerciseTime" => ("exercise_time", to_minutes),
        "FlightsClimbed" => ("flights_climbed", to_count),
        "WalkingSpeed" => ("walking_speed", to_metres_per_second),
        "WalkingStepLength" => ("walking_step_length", to_metres),
        "WalkingAsymmetryPercentage" => ("walking_asymmetry_percentage", to_percent),
        "TimeInDaylight" => ("time_in_daylight", to_minutes),
        _ => return None,
    })
}

fn normalize_unit(unit: &str) -> String {
    let mut out = String::new();
    for ch in unit.trim().chars() {
        if ch.is_whitespace() {
            continue;
        }
        for lower in ch.to_lowercase() {
            if lower == '·' {
                out.push('*');
            } else {
                out.push(lower);
            }
        }
    }
    out
}

fn passthrough(value: f64, _unit: &str) -> Option<f64> {
    Some(value)
}

fn per_minute(value: f64, unit: &str) -> Option<f64> {
    let unit = normalize_unit(unit);
    if unit == "count/min" || unit == "count/minute" || unit == "bpm" || unit.is_empty() {
        Some(value)
    } else {
        None
    }
}

fn to_count(value: f64, unit: &str) -> Option<f64> {
    let unit = normalize_unit(unit);
    if unit == "count" || unit.is_empty() { Some(value) } else { None }
}

fn to_kilograms(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "kg" => Some(value),
        "g" => Some(value / 1000.0),
        "lb" => Some(value * 0.45359237),
        "st" => Some(value * 6.35029318),
        _ => None,
    }
}

fn to_centimeters(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "cm" => Some(value),
        "m" => Some(value * 100.0),
        "mm" => Some(value / 10.0),
        "in" => Some(value * 2.54),
        "ft" => Some(value * 30.48),
        _ => None,
    }
}

fn to_metres(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "m" => Some(value),
        "km" => Some(value * 1000.0),
        "cm" => Some(value / 100.0),
        "mi" => Some(value * 1609.344),
        "yd" => Some(value * 0.9144),
        "ft" => Some(value * 0.3048),
        "in" => Some(value * 0.0254),
        _ => None,
    }
}

fn to_metres_per_second(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "m/s" => Some(value),
        "km/hr" | "km/h" => Some(value / 3.6),
        "mi/hr" | "mi/h" => Some(value * 0.44704),
        "ft/s" => Some(value * 0.3048),
        _ => None,
    }
}

fn to_milliseconds(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "ms" => Some(value),
        "s" | "sec" => Some(value * 1000.0),
        _ => None,
    }
}

fn to_minutes(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "min" => Some(value),
        "s" | "sec" => Some(value / 60.0),
        "hr" | "h" => Some(value * 60.0),
        _ => None,
    }
}

fn to_kilocalories(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "kcal" | "cal" => Some(value),
        "kj" => Some(value / 4.184),
        _ => None,
    }
}

fn to_celsius(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "degc" | "°c" | "c" => Some(value),
        "degf" | "°f" | "f" => Some(((value - 32.0) * 5.0) / 9.0),
        _ => None,
    }
}

fn to_watts(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "w" => Some(value),
        "kw" => Some(value * 1000.0),
        _ => None,
    }
}

fn to_millimetres_mercury(value: f64, unit: &str) -> Option<f64> {
    if normalize_unit(unit) == "mmhg" { Some(value) } else { None }
}

fn to_mets(value: f64, unit: &str) -> Option<f64> {
    match normalize_unit(unit).as_str() {
        "kcal/hr*kg" | "met" | "mets" => Some(value),
        _ => None,
    }
}

fn to_percent(value: f64, unit: &str) -> Option<f64> {
    let unit = normalize_unit(unit);
    if unit == "%" || unit == "percent" || unit.is_empty() {
        Some(if value <= 1.0 { value * 100.0 } else { value })
    } else {
        None
    }
}

fn parse_f64(raw: &str) -> Option<f64> {
    let value: f64 = raw.trim().parse().ok()?;
    if value.is_finite() { Some(value) } else { None }
}

fn js_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    (value + 0.5).floor()
}

fn round1(value: f64) -> f64 {
    js_round(value * 10.0) / 10.0
}

fn round2(value: f64) -> f64 {
    js_round(value * 100.0) / 100.0
}

fn round3(value: f64) -> f64 {
    js_round(value * 1000.0) / 1000.0
}

fn num(value: f64) -> Value {
    serde_json::Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
}

fn parse_apple_date(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if is_full_iso(raw) {
        return Some(raw.to_string());
    }
    let bytes = raw.as_bytes();
    if bytes.len() < 19 || !digits(bytes, 0, 4) || bytes[4] != b'-' || !digits(bytes, 5, 2) || bytes[7] != b'-' || !digits(bytes, 8, 2) {
        return None;
    }
    if bytes[10] != b' ' && bytes[10] != b'T' {
        return None;
    }
    if !digits(bytes, 11, 2) || bytes[13] != b':' || !digits(bytes, 14, 2) || bytes[16] != b':' || !digits(bytes, 17, 2) {
        return None;
    }
    let rest = raw[19..].trim_start();
    let off = rest.as_bytes();
    if off.len() < 3 || (off[0] != b'+' && off[0] != b'-') || !digits(off, 1, 2) {
        return None;
    }
    let minutes = if off.len() == 3 {
        "00".to_string()
    } else if off.len() == 5 && digits(off, 3, 2) {
        rest[3..5].to_string()
    } else if off.len() == 6 && off[3] == b':' && digits(off, 4, 2) {
        rest[4..6].to_string()
    } else {
        return None;
    };
    Some(format!(
        "{}-{}-{}T{}:{}:{}{}:{}",
        &raw[0..4], &raw[5..7], &raw[8..10], &raw[11..13], &raw[14..16], &raw[17..19], &rest[0..3], minutes
    ))
}

fn is_full_iso(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() < 20 || !digits(bytes, 0, 4) || bytes[4] != b'-' || !digits(bytes, 5, 2) || bytes[7] != b'-' || !digits(bytes, 8, 2) || bytes[10] != b'T' {
        return false;
    }
    if !digits(bytes, 11, 2) || bytes[13] != b':' || !digits(bytes, 14, 2) || bytes[16] != b':' || !digits(bytes, 17, 2) {
        return false;
    }
    let mut index = 19;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    if index >= bytes.len() {
        return false;
    }
    if bytes[index] == b'Z' {
        return index + 1 == bytes.len();
    }
    if (bytes[index] == b'+' || bytes[index] == b'-') && index + 6 == bytes.len() && digits(bytes, index + 1, 2) && bytes[index + 3] == b':' && digits(bytes, index + 4, 2) {
        return true;
    }
    false
}

fn digits(bytes: &[u8], start: usize, len: usize) -> bool {
    bytes.len() >= start + len && bytes[start..start + len].iter().all(|byte| byte.is_ascii_digit())
}

fn epoch_ms(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso).ok().map(|time| time.timestamp_millis())
}

fn timestamp_receipt(raw: &str, zone: Option<&str>) -> Value {
    let mut receipt = Map::new();
    receipt.insert("timestamp_text".into(), json!(raw));
    receipt.insert("timestamp_basis".into(), json!("source_offset_timestamp"));
    if let Some(offset) = trailing_offset(raw.trim()) {
        receipt.insert("utc_offset_text".into(), json!(offset));
        receipt.insert("utc_offset_meaning".into(), json!("meaning_not_documented"));
    } else {
        receipt.insert("utc_offset_missing_reason".into(), json!("not_provided_by_source"));
    }
    if let Some(zone) = zone {
        receipt.insert("source_time_zone".into(), json!(zone));
        receipt.insert(
            "source_time_zone_meaning".into(),
            json!("apple_health_object_creation_time_zone"),
        );
    }
    Value::Object(receipt)
}

fn trailing_offset(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.last().is_some_and(|byte| *byte == b'Z' || *byte == b'z') {
        return Some(raw[raw.len() - 1..].to_string());
    }
    if raw.len() >= 6 {
        let tail = &raw[raw.len() - 6..];
        let tail_bytes = tail.as_bytes();
        if (tail_bytes[0] == b'+' || tail_bytes[0] == b'-') && digits(tail_bytes, 1, 2) && tail_bytes[3] == b':' && digits(tail_bytes, 4, 2) {
            return Some(tail.to_string());
        }
    }
    if raw.len() >= 5 {
        let tail = &raw[raw.len() - 5..];
        let tail_bytes = tail.as_bytes();
        if (tail_bytes[0] == b'+' || tail_bytes[0] == b'-') && digits(tail_bytes, 1, 4) {
            return Some(tail.to_string());
        }
    }
    None
}

fn at_offset_of(reference: &str, instant_ms: f64) -> String {
    let rounded = js_round(instant_ms) as i64;
    match offset_colon(reference) {
        None => format_utc(rounded),
        Some(offset) => {
            let shifted = format_utc(rounded + offset_to_ms(&offset));
            format!("{}{}", &shifted[..23], offset)
        }
    }
}

fn offset_colon(reference: &str) -> Option<String> {
    if reference.len() < 6 {
        return None;
    }
    let tail = &reference[reference.len() - 6..];
    let bytes = tail.as_bytes();
    if (bytes[0] == b'+' || bytes[0] == b'-') && digits(bytes, 1, 2) && bytes[3] == b':' && digits(bytes, 4, 2) {
        Some(tail.to_string())
    } else {
        None
    }
}

fn offset_to_ms(offset: &str) -> i64 {
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let hours: i64 = offset[1..3].parse().unwrap_or(0);
    let minutes: i64 = offset[4..6].parse().unwrap_or(0);
    sign * (hours * 60 + minutes) * 60_000
}

fn format_utc(millis: i64) -> String {
    let time = DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH);
    time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn clock_ms(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    let tee = bytes.iter().position(|byte| *byte == b'T')?;
    let index = tee + 1;
    if index + 8 > bytes.len() {
        return None;
    }
    if !(bytes[index].is_ascii_digit()
        && bytes[index + 1].is_ascii_digit()
        && bytes[index + 2] == b':'
        && bytes[index + 3].is_ascii_digit()
        && bytes[index + 4].is_ascii_digit()
        && bytes[index + 5] == b':'
        && bytes[index + 6].is_ascii_digit()
        && bytes[index + 7].is_ascii_digit())
    {
        return None;
    }
    let hour = two(bytes, index);
    let minute = two(bytes, index + 3);
    let second = two(bytes, index + 6);
    let mut fraction = 0;
    if index + 8 < bytes.len() && bytes[index + 8] == b'.' {
        let mut cursor = index + 9;
        let start = cursor;
        while cursor < bytes.len() && cursor - start < 3 && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor > start {
            let mut digits = std::str::from_utf8(&bytes[start..cursor]).unwrap_or("").to_string();
            while digits.len() < 3 {
                digits.push('0');
            }
            fraction = digits.parse().unwrap_or(0);
        }
    }
    Some((hour * 3600 + minute * 60 + second) * 1000 + fraction)
}

fn two(bytes: &[u8], index: usize) -> i64 {
    (bytes[index] - b'0') as i64 * 10 + (bytes[index + 1] - b'0') as i64
}

fn beat_offset_ms(recorded_at: &str, ends_at: Option<&str>, time_of_day: &str) -> Option<i64> {
    let beat_within_hour = beat_within_hour(time_of_day)?;
    let start_ms = clock_ms(recorded_at)?;
    let end_ms = ends_at.and_then(clock_ms);
    let duration_ms = end_ms.map(|end| (end - start_ms + DAY_MS) % DAY_MS).unwrap_or(0);
    if duration_ms > LONGEST_PLACEABLE_WINDOW_MS {
        return None;
    }
    let base = beat_within_hour - (start_ms % HOUR_MS);
    let mut best: Option<i64> = None;
    for offset in [base, base + HOUR_MS, base - HOUR_MS] {
        if offset < -BEAT_WINDOW_TOLERANCE_MS || offset > duration_ms + BEAT_WINDOW_TOLERANCE_MS {
            continue;
        }
        if best.is_none_or(|current| offset.abs() < current.abs()) {
            best = Some(offset);
        }
    }
    best
}

fn beat_within_hour(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            if let Some(value) = beat_at(bytes, index) {
                return Some(value);
            }
        }
        index += 1;
    }
    None
}

fn beat_at(bytes: &[u8], index: usize) -> Option<i64> {
    for len in [2usize, 1] {
        if index + len > bytes.len() || !bytes[index..index + len].iter().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let colon = index + len;
        if colon + 5 >= bytes.len() || bytes[colon] != b':' || bytes[colon + 3] != b':' {
            continue;
        }
        if !(bytes[colon + 1].is_ascii_digit()
            && bytes[colon + 2].is_ascii_digit()
            && bytes[colon + 4].is_ascii_digit()
            && bytes[colon + 5].is_ascii_digit())
        {
            continue;
        }
        let minute = two(bytes, colon + 1);
        let second = two(bytes, colon + 4);
        let mut fraction = 0;
        let mut cursor = colon + 6;
        if cursor < bytes.len() && (bytes[cursor] == b'.' || bytes[cursor] == b',') {
            cursor += 1;
            let start = cursor;
            while cursor < bytes.len() && cursor - start < 3 && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor > start {
                let mut digits = std::str::from_utf8(&bytes[start..cursor]).unwrap_or("").to_string();
                while digits.len() < 3 {
                    digits.push('0');
                }
                fraction = digits.parse().unwrap_or(0);
            }
        }
        return Some(minute * 60_000 + second * 1000 + fraction);
    }
    None
}

struct DeviceFields {
    name: Option<String>,
    manufacturer: Option<String>,
    model: Option<String>,
    hardware: Option<String>,
}

fn parse_apple_device(raw: &str) -> Option<DeviceFields> {
    if raw.is_empty() {
        return None;
    }
    let mut body = strip_hk_prefix(raw).unwrap_or_else(|| raw.to_string());
    let trimmed = body.trim_end();
    if let Some(stripped) = trimmed.strip_suffix('>') {
        body = stripped.to_string();
    }
    let hits = field_hits(&body);
    if hits.is_empty() {
        return None;
    }
    let mut fields: HashMap<&str, String> = HashMap::new();
    for (index, hit) in hits.iter().enumerate() {
        let end = if index + 1 < hits.len() { hits[index + 1].0 } else { body.len() };
        let value = body[hit.2..end].trim();
        if !value.is_empty() {
            fields.insert(hit.1, value.to_string());
        }
    }
    let device = DeviceFields {
        name: fields.get("name").cloned(),
        manufacturer: fields.get("manufacturer").cloned(),
        model: fields.get("model").cloned(),
        hardware: fields.get("hardware").cloned(),
    };
    if device.name.is_none() && device.manufacturer.is_none() && device.model.is_none() && device.hardware.is_none() {
        None
    } else {
        Some(device)
    }
}

fn strip_hk_prefix(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if !raw.starts_with("<<HKDevice:") {
        return None;
    }
    let mut index = "<<HKDevice:".len();
    while index < bytes.len() && bytes[index] != b'>' {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }
    index += 1;
    if index < bytes.len() && bytes[index] == b',' {
        index += 1;
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    Some(raw[index..].to_string())
}

/// `(match start, field name, value start)`.
fn field_hits(body: &str) -> Vec<(usize, &'static str, usize)> {
    let bytes = body.as_bytes();
    let mut hits = Vec::new();
    if let Some((name, value_start)) = field_at(body, 0) {
        hits.push((0, name, value_start));
    }
    for index in 0..bytes.len() {
        if bytes[index] != b',' {
            continue;
        }
        let mut cursor = index + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if let Some((name, value_start)) = field_at(body, cursor) {
            hits.push((index, name, value_start));
        }
    }
    hits
}

fn field_at(body: &str, at: usize) -> Option<(&'static str, usize)> {
    for field in DEVICE_FIELDS {
        let after = at + field.len();
        if body[at..].starts_with(field) && body.as_bytes().get(after) == Some(&b':') {
            return Some((*field, after + 1));
        }
    }
    None
}

fn local_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn attr_map(event: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in event.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let value = attr.unescape_value().map(|text| text.into_owned()).unwrap_or_default();
        map.insert(key, value);
    }
    map
}

impl Clone for SleepFragment {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            device_raw: self.device_raw.clone(),
            on_day: self.on_day.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            input_start: self.input_start.clone(),
            input_end: self.input_end.clone(),
            value: self.value.clone(),
            source_time_zone: self.source_time_zone.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{beat_offset_ms, import_apple_str, parse_apple_date, parse_apple_device};
    use std::fs;
    use std::path::Path;

    #[test]
    fn apple_date_keeps_the_offset() {
        assert_eq!(
            parse_apple_date("2026-08-09 06:12:00 -0700").as_deref(),
            Some("2026-08-09T06:12:00-07:00")
        );
        assert_eq!(parse_apple_date("9 Aug 2026, 6:12am"), None);
    }

    #[test]
    fn device_string_keeps_the_comma_inside_hardware() {
        let raw = "<<HKDevice: 0x2809b6800>, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch6,2, software:10.2>";
        let device = parse_apple_device(raw).unwrap();
        assert_eq!(device.hardware.as_deref(), Some("Watch6,2"));
        assert_eq!(device.name.as_deref(), Some("Apple Watch"));
        assert!(parse_apple_device("<<HKDevice: 0x1>, software:10.2>").is_none());
    }

    #[test]
    fn beat_minute_survives_a_twelve_hour_clock() {
        assert_eq!(
            beat_offset_ms("2026-08-09T06:12:00-07:00", Some("2026-08-09T06:13:01-07:00"), "6:12:30.50"),
            Some(30_500)
        );
        assert_eq!(
            beat_offset_ms("2020-11-15T23:33:17-05:00", Some("2020-11-15T23:34:18-05:00"), "10:33:19.09 PM"),
            Some(2_090)
        );
    }

    #[test]
    fn sdnn_and_rmssd_stay_separate_and_sleep_keeps_its_offset() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/exports/apple/export.xml");
        let xml = fs::read_to_string(path).unwrap();
        let parsed = import_apple_str(&xml).unwrap();
        let events = parsed["events"].as_array().unwrap();
        let sdnn: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["kind"] == "series" && event["quantity"] == "hrv_sdnn")
            .collect();
        assert_eq!(sdnn.len(), 2);
        assert_eq!(sdnn[0]["sample"]["value"], 52.3);
        assert_eq!(sdnn[0]["beat_samples"][0]["at"], "2026-08-09T06:12:00.220-07:00");
        assert_eq!(sdnn[0]["beat_samples"][0]["value"], 869.6);
        assert_eq!(sdnn[0]["derived"]["signal"]["value"], 50.6);
        assert_eq!(sdnn[0]["derived"]["signal"]["type"], "hrv_rmssd");
        assert_eq!(sdnn[0]["derived"]["signal"]["derived"]["from"], "hrv_beats");
        assert_eq!(sdnn[0]["derived"]["signal"]["derived"]["window_s"], 61.1);
        assert_eq!(sdnn[0]["derived"]["signal"]["derived"]["n_beats"], 65);
        assert!(sdnn[1].get("derived").is_none());
        assert_eq!(sdnn[1]["beat_samples"].as_array().unwrap().len(), 4);

        let temperature = events
            .iter()
            .find(|event| event["kind"] == "point" && event["type"] == "body_temperature")
            .unwrap();
        assert_eq!(temperature["writer"], "Health");
        assert_eq!(temperature["value"], 36.7);

        let watch_sleep = parsed["sleep_sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["writer"] == "Apple Watch")
            .unwrap();
        assert_eq!(watch_sleep["start"], "2026-08-08T22:15:00-07:00");
        assert_eq!(watch_sleep["end"], "2026-08-09T06:05:00-07:00");
        assert_eq!(watch_sleep["aggregates"]["efficiency_pct"], 92.6);
        assert_eq!(watch_sleep["receipt"]["input_start_timestamp"]["utc_offset_text"], "-0700");
        let oura_sleep = parsed["sleep_sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["writer"] == "Oura")
            .unwrap();
        assert_ne!(oura_sleep["writer"], watch_sleep["writer"]);

        let summary = events.iter().find(|event| event["kind"] == "activity").unwrap();
        assert_eq!(summary["writer"], "Apple Health");
        assert_eq!(summary["summaries"][0]["summary_type"], "active_energy_burned");
        assert_eq!(summary["summaries"][0]["value"], 612.4);
        assert!(summary["summaries"][0].get("recorded_at").is_none());
    }

    #[test]
    fn a_hand_entered_night_does_not_join_the_watch_night() {
        let xml = r#"<HealthData>
          <Record type="HKCategoryTypeIdentifierSleepAnalysis" sourceName="Health" startDate="2026-08-08 22:00:00 -0700" endDate="2026-08-09 06:00:00 -0700" value="HKCategoryValueSleepAnalysisInBed"/>
          <Record type="HKCategoryTypeIdentifierSleepAnalysis" sourceName="Apple Watch" startDate="2026-08-08 22:10:00 -0700" endDate="2026-08-09 06:00:00 -0700" value="HKCategoryValueSleepAnalysisInBed"/>
        </HealthData>"#;
        let parsed = import_apple_str(xml).unwrap();
        let sessions = parsed["sleep_sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["writer"], "Health");
        assert_eq!(sessions[1]["writer"], "Apple Watch");
    }
}
