//! Map a source whose reader already produced named fields.
//!
//! A mapping file is markdown. The program reads the first fenced block tagged
//! `ath-mapping`. Each line names their field and our field. A mapped field becomes
//! the typed value. Every other field stays in `supplied_fields` under its name.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::{json, Map, Number, Value};

use crate::store::{GroupOutcome, GroupRecord, Store};

pub const REFUSE_PERSONAL_REPORT: &str = "refusing to turn a personal report into a measured record";
pub const ENVELOPE_NOT_TIMESERIES: &str =
    "envelope is not a timeseries page: pass its data array under workout or sleep";

const SECTIONS: &[&str] = &["sleep", "workout", "timeseries"];
const SETTINGS: &[&str] = &["id_field", "start_field", "end_field", "time_field", "value_field"];
const BUILT_IN_ROUTES: &[&str] = &[
    "garmin_account_export",
    "garmin_personal_api",
    "strava_bulk",
    "apple_health",
    "whoop_csv",
    "oura_csv",
    "manual_entry",
    "unknown_import",
];
const BUILT_IN_METHODS: &[&str] = &[
    "garmin_account/v1",
    "garmin_personal/v1",
    "strava_bulk/v1",
    "apple_health/v1",
    "whoop_csv/v1",
    "oura_csv/v1",
    "manual_entry/v1",
    "unknown_import/v1",
];

/// `POINT_MEASUREMENT_UNITS` in `src/v1/schema.ts`. A test keeps the two equal.
pub const POINT_UNITS: &[(&str, &str)] = &[
    ("hrv_rmssd", "ms"),
    ("hrv_sdnn", "ms"),
    ("resting_heart_rate", "bpm"),
    ("body_weight", "kg"),
    ("respiratory_rate", "brpm"),
    ("vo2_max", "ml/kg/min"),
    ("hr_recovery", "bpm"),
    ("walking_heart_rate", "bpm"),
    ("oxygen_saturation", "%"),
    ("body_temperature", "°C"),
    ("skin_temperature", "°C"),
    ("wrist_temperature_sleeping", "°C"),
    ("temperature_deviation", "°C"),
    ("height", "cm"),
    ("lean_body_mass", "kg"),
    ("body_fat_percentage", "%"),
    ("blood_pressure_systolic", "mmHg"),
    ("blood_pressure_diastolic", "mmHg"),
];

const SLEEP_FIELDS: &[(&str, &str)] = &[
    ("asleep_seconds", "s"),
    ("in_bed_seconds", "s"),
    ("interruptions", "count"),
    ("stage_minutes.awake", "min"),
    ("stage_minutes.light", "min"),
    ("stage_minutes.deep", "min"),
    ("stage_minutes.rem", "min"),
];
const WORKOUT_TEXT: &[&str] = &["activity_type", "name", "description", "text_result"];
const WORKOUT_SUMMARIES: &[(&str, &str)] = &[
    ("source_reported_summaries.distance", "m"),
    ("source_reported_summaries.elevation_gain", "m"),
    ("source_reported_summaries.energy", "kcal"),
];
const PERSONAL_SECTIONS: &[&str] = &["athlete_report", "nutrition_entry"];
const PERSONAL_FIELDS: &[&str] = &["source_id", "report_type", "reported_via", "rating"];
const EFFICIENCY_FIELDS: &[&str] = &[
    "efficiency",
    "efficiency_pct",
    "sleep_efficiency",
    "sleep-efficiency",
    "source_reported_summaries.sleep_efficiency",
];
const RMSSD_NAMES: &[&str] = &["heart_rate_variability_rmssd"];
const SDNN_NAMES: &[&str] = &["heart_rate_variability_sdnn", "hrv_mean_sdnn"];

#[derive(Debug, Clone, Default)]
pub struct MappingFile {
    pub name: String,
    pub route: String,
    pub writer_field: String,
    pub sensor: Vec<String>,
    pub device_name: Vec<String>,
    pub device_model: Vec<String>,
    pub device_hardware: Vec<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Default)]
pub struct Section {
    pub name: String,
    pub settings: HashMap<String, String>,
    pub lines: Vec<MapLine>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapLine {
    pub their_field: String,
    pub our_field: String,
    pub units: Option<(String, String)>,
    pub basis: Option<String>,
    pub scale: Option<String>,
    pub when_absent: bool,
    pub when_writer: Option<Vec<String>>,
}

impl MappingFile {
    fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }
}

impl Section {
    fn setting(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(String::as_str)
    }
}

/// Read the first `ath-mapping` block. A trailing `\r` on a line is removed.
pub fn parse_mapping(text: &str) -> Result<MappingFile, String> {
    let lines: Vec<&str> = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
    let start = lines
        .iter()
        .position(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("```") && trimmed.trim_start_matches('`').trim() == "ath-mapping"
        })
        .ok_or("the mapping file has no ath-mapping block")?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim().starts_with("```"))
        .map(|offset| start + 1 + offset)
        .ok_or("the ath-mapping block is not closed")?;
    let mut file = MappingFile::default();
    let mut current: Option<Section> = None;
    for raw in &lines[start + 1..end] {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            let name = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| format!("could not read '{line}'"))?;
            if let Some(section) = current.take() {
                file.sections.push(section);
            }
            current = Some(Section { name: name.to_string(), ..Section::default() });
            continue;
        }
        let (key, rest) = line.split_once(':').ok_or_else(|| format!("could not read '{line}'"))?;
        let key = key.trim();
        let rest = rest.trim();
        if key.is_empty() || rest.is_empty() {
            return Err(format!("could not read '{line}'"));
        }
        match current.as_mut() {
            None => {
                let list = || rest.split(',').map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect::<Vec<_>>();
                match key {
                    "name" => file.name = rest.to_string(),
                    "route" => file.route = rest.to_string(),
                    "writer_field" => file.writer_field = rest.to_string(),
                    "sensor" => file.sensor = list(),
                    "device_name" => file.device_name = list(),
                    "device_model" => file.device_model = list(),
                    "device_hardware" => file.device_hardware = list(),
                    _ => return Err(format!("could not read '{line}'")),
                }
            }
            Some(section) => {
                if SETTINGS.contains(&key) {
                    section.settings.insert(key.to_string(), rest.to_string());
                } else {
                    section.lines.push(parse_line(line, key, rest)?);
                }
            }
        }
    }
    if let Some(section) = current.take() {
        file.sections.push(section);
    }
    Ok(file)
}

fn parse_line(line: &str, their_field: &str, rest: &str) -> Result<MapLine, String> {
    let unreadable = || format!("could not read '{line}'");
    let mut tokens = rest.split_whitespace();
    let our_field = tokens.next().ok_or_else(unreadable)?;
    let mut parsed = MapLine {
        their_field: their_field.to_string(),
        our_field: our_field.to_string(),
        ..MapLine::default()
    };
    let mut units = Vec::new();
    let mut keywords_started = false;
    while let Some(token) = tokens.next() {
        match token {
            "basis" | "scale" | "when-writer" => {
                keywords_started = true;
                let value = tokens.next().ok_or_else(unreadable)?.to_string();
                let slot_taken = match token {
                    "basis" => parsed.basis.replace(value).is_some(),
                    "scale" => parsed.scale.replace(value).is_some(),
                    _ => parsed
                        .when_writer
                        .replace(value.split(',').map(str::to_string).collect())
                        .is_some(),
                };
                if slot_taken {
                    return Err(unreadable());
                }
            }
            "when-absent" => {
                keywords_started = true;
                if parsed.when_absent {
                    return Err(unreadable());
                }
                parsed.when_absent = true;
            }
            unit if !keywords_started => units.push(unit.to_string()),
            _ => return Err(unreadable()),
        }
    }
    match units.len() {
        0 => {}
        2 => parsed.units = Some((units[0].clone(), units[1].clone())),
        _ => return Err(unreadable()),
    }
    if parsed
        .when_writer
        .as_ref()
        .is_some_and(|writers| writers.iter().any(|writer| writer.trim().is_empty()))
    {
        return Err(unreadable());
    }
    Ok(parsed)
}

fn point_unit(quantity: &str) -> Option<&'static str> {
    POINT_UNITS.iter().find(|(name, _)| *name == quantity).map(|(_, unit)| *unit)
}

fn statistic_name(their_field: &str) -> &str {
    their_field.strip_prefix("type.").unwrap_or(their_field)
}

enum Target {
    SleepField(&'static str),
    WorkoutText,
    WorkoutSummary(String, &'static str),
    Point(&'static str),
    Summary(String),
    Score(String),
}

fn target(section: &str, our_field: &str) -> Option<Target> {
    if let Some(summary) = our_field.strip_prefix("provider_summary.") {
        return (!summary.is_empty()).then(|| Target::Summary(summary.to_string()));
    }
    if let Some(metric) = our_field.strip_prefix("provider_score.") {
        return (!metric.is_empty()).then(|| Target::Score(metric.to_string()));
    }
    if let Some(unit) = point_unit(our_field) {
        return Some(Target::Point(unit));
    }
    if section == "sleep" {
        if let Some((_, unit)) = SLEEP_FIELDS.iter().find(|(name, _)| *name == our_field) {
            return Some(Target::SleepField(unit));
        }
    }
    if section == "workout" {
        if WORKOUT_TEXT.contains(&our_field) {
            return Some(Target::WorkoutText);
        }
        if let Some((_, unit)) = WORKOUT_SUMMARIES.iter().find(|(name, _)| *name == our_field) {
            return Some(Target::WorkoutSummary(
                our_field.trim_start_matches("source_reported_summaries.").to_string(),
                unit,
            ));
        }
    }
    None
}

/// Check a mapping before anything is written. The first problem found is returned.
pub fn check_mapping(file: &MappingFile) -> Result<(), String> {
    for section in &file.sections {
        if PERSONAL_SECTIONS.contains(&section.name.as_str())
            || section.lines.iter().any(|line| PERSONAL_FIELDS.contains(&line.our_field.as_str()))
        {
            return Err(REFUSE_PERSONAL_REPORT.into());
        }
    }
    for line in file.sections.iter().flat_map(|section| section.lines.iter()) {
        refuse_line(line)?;
    }
    for (key, value) in [("name", &file.name), ("route", &file.route), ("writer_field", &file.writer_field)] {
        if value.is_empty() {
            return Err(format!("the mapping file needs {key}"));
        }
    }
    if BUILT_IN_ROUTES.contains(&file.route.as_str()) {
        return Err(format!("route {} is already used by a built-in importer", file.route));
    }
    let method = format!("{}/v1", file.route);
    if BUILT_IN_METHODS.contains(&method.as_str()) {
        return Err(format!("match key method {method} is already used by a built-in importer"));
    }
    for section in &file.sections {
        if !SECTIONS.contains(&section.name.as_str()) {
            return Err(format!("unknown mapping section {}", section.name));
        }
        let mut when_absent = HashSet::new();
        let mut fallbacks = HashSet::new();
        for line in &section.lines {
            check_line(&section.name, line)?;
            if line.when_absent && !when_absent.insert(line.our_field.clone()) {
                return Err(format!("two when-absent lines write {}", line.our_field));
            }
            if line.when_writer.is_none() && !fallbacks.insert(line.their_field.clone()) {
                return Err(format!("two lines read {} without when-writer", line.their_field));
            }
        }
    }
    Ok(())
}

/// The four refusals in D179 that come from a single line.
fn refuse_line(line: &MapLine) -> Result<(), String> {
    let our = line.our_field.as_str();
    let name = statistic_name(&line.their_field);
    if our == "hrv_rmssd" || our == "hrv_sdnn" {
        let named = if our == "hrv_rmssd" {
            RMSSD_NAMES.contains(&name)
                || (name == "average_hrv"
                    && line.basis.as_deref() == Some("hrv_mean_rmssd")
                    && line.when_writer.is_some())
        } else {
            SDNN_NAMES.contains(&name)
        };
        if !named {
            return Err(format!(
                "refusing to map {name} onto {our}; the field does not name the statistic"
            ));
        }
    }
    if EFFICIENCY_FIELDS.contains(&our) {
        return Err(format!(
            "refusing to map {} onto {our}; efficiency is computed from asleep_seconds and in_bed_seconds",
            line.their_field
        ));
    }
    if our == "maximum_heart_rate_bpm" {
        return Err(format!(
            "refusing to map {} onto maximum_heart_rate_bpm; that field is the athlete's known maximum",
            line.their_field
        ));
    }
    Ok(())
}

fn check_line(section: &str, line: &MapLine) -> Result<(), String> {
    let our = line.our_field.as_str();
    let Some(kind) = target(section, our) else {
        return Err(format!("unknown field {our}"));
    };
    if let Some(basis) = &line.basis {
        let heart_rate_summary = matches!(kind, Target::Summary(_))
            && line.units.as_ref().is_some_and(|(_, unit)| unit == "bpm");
        if !(our == "hrv_rmssd" || our == "hrv_sdnn" || heart_rate_summary) {
            return Err("basis is only used for hrv_rmssd, hrv_sdnn, and heart-rate summaries".into());
        }
        if !VALUE_BASES.contains(&basis.as_str()) {
            return Err(format!("unknown basis {basis}"));
        }
    }
    match (&kind, &line.scale) {
        (Target::Score(_), None) => return Err(format!("provider_score {} needs a scale", line.their_field)),
        (Target::Score(_), Some(_)) | (_, None) => {}
        (_, Some(_)) => return Err("scale is only used for provider_score".into()),
    }
    if matches!(kind, Target::WorkoutText) {
        if line.units.is_some() {
            return Err(format!("{our} takes no unit"));
        }
        return Ok(());
    }
    let Some((their_unit, our_unit)) = &line.units else {
        return Err(format!("{our} needs a unit pair"));
    };
    match kind {
        Target::Point(unit) | Target::SleepField(unit) | Target::WorkoutSummary(_, unit) if our_unit != unit => {
            return Err(format!("{our} uses {unit}"));
        }
        _ => {}
    }
    convert_unit(1.0, their_unit, our_unit)
        .map_err(|_| format!("cannot convert {their_unit} to {our_unit} for {}", line.their_field))?;
    Ok(())
}

/// Values a mapping line may name with `basis`. They are the `field_basis` values.
pub const VALUE_BASES: &[&str] = &["provider_or_open_wearables", "provider", "hrv_mean_rmssd"];

/// Convert by a fixed pair (D180). A whole result is an integer JSON number.
pub fn convert_unit(value: f64, their_unit: &str, our_unit: &str) -> Result<Value, String> {
    let same = matches!(
        (their_unit, our_unit),
        ("s", "s")
            | ("min", "min")
            | ("ms", "ms")
            | ("bpm", "bpm")
            | ("m", "m")
            | ("kcal", "kcal")
            | ("count", "count")
            | ("score", "score")
            | ("brpm", "brpm")
            | ("%", "%")
            | ("percent", "%")
            | ("celsius", "°C")
            | ("ml_kg_min", "ml/kg/min")
            | ("percent", "score")
    );
    let result = if same {
        value
    } else if their_unit == "s" && our_unit == "min" {
        value / 60.0
    } else {
        return Err(format!("cannot convert {their_unit} to {our_unit}"));
    };
    Ok(number_value(result))
}

fn number_value(value: f64) -> Value {
    const SAFE: f64 = 9_007_199_254_740_991.0;
    if value.fract() == 0.0 && value.abs() <= SAFE {
        return Value::Number(Number::from(value as i64));
    }
    Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
}

/// The receipt for a timestamp exactly as the source wrote it.
pub fn input_receipt(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.ends_with('Z') || trimmed.ends_with('z') {
        return json!({
            "timestamp_text": text,
            "utc_offset_text": "Z",
            "utc_offset_meaning": "documented_recording_offset",
            "timestamp_basis": "source_instant"
        });
    }
    let offset = trimmed.get(trimmed.len().saturating_sub(6)..).unwrap_or("");
    json!({
        "timestamp_text": text,
        "utc_offset_text": offset,
        "utc_offset_meaning": "meaning_not_documented",
        "timestamp_basis": "source_offset_timestamp"
    })
}

fn utc_text(text: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(text.trim())
        .ok()
        .map(|time| crate::time::format_datetime(time.with_timezone(&chrono::Utc)))
}

/// Escape one part of a match key, then the parts are joined with `:`.
pub fn key_part(part: &str) -> String {
    part.replace('\\', "\\\\").replace(':', "\\:")
}

fn match_key(parts: &[&str]) -> String {
    parts.iter().map(|part| key_part(part)).collect::<Vec<_>>().join(":")
}

fn path_value<'a>(object: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut current = object.get(parts.next()?)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn path_text(object: &Map<String, Value>, path: &str) -> Option<String> {
    match path_value(object, path)? {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Remove used leaves. An object left empty is removed too.
fn remove_path(object: &mut Map<String, Value>, path: &str) {
    match path.split_once('.') {
        None => {
            object.shift_remove(path);
        }
        Some((head, rest)) => {
            let empty = match object.get_mut(head) {
                Some(Value::Object(child)) => {
                    remove_path(child, rest);
                    child.is_empty()
                }
                _ => false,
            };
            if empty {
                object.shift_remove(head);
            }
        }
    }
}

struct Run<'a> {
    file: &'a MappingFile,
    received_at: String,
    write: bool,
    record_ids: Vec<String>,
    understood: BTreeSet<String>,
    preserved: BTreeSet<String>,
    left_out: Vec<String>,
    keys: HashSet<String>,
    devices: HashSet<(String, String)>,
}

struct Device {
    writer: String,
    sensor: String,
    fields: Map<String, Value>,
}

impl Run<'_> {
    fn device(&self, row: &Map<String, Value>, writer: &str) -> Device {
        let sensor = self
            .file
            .sensor
            .iter()
            .filter_map(|path| path_text(row, path))
            .collect::<Vec<_>>()
            .join("|");
        let first = |paths: &[String]| paths.iter().find_map(|path| path_text(row, path));
        let mut fields = Map::new();
        for (key, paths) in [
            ("name", &self.file.device_name),
            ("model", &self.file.device_model),
            ("hardware", &self.file.device_hardware),
        ] {
            if let Some(value) = first(paths) {
                fields.insert(key.into(), json!(value));
            }
        }
        Device { writer: writer.to_string(), sensor, fields }
    }

    fn used_paths(&self) -> Vec<String> {
        let mut paths = vec![self.file.writer_field.clone()];
        for list in [&self.file.sensor, &self.file.device_name, &self.file.device_model, &self.file.device_hardware] {
            paths.extend(list.iter().cloned());
        }
        paths
    }

    fn source_group(
        &mut self,
        store: &mut Store,
        device: &Device,
    ) -> Result<(String, Vec<GroupRecord>), String> {
        if !self.write {
            return Ok(("preview".into(), Vec::new()));
        }
        let source_id = store.ensure_mapped_source(&device.writer, &self.file.route, &device.writer, &device.sensor)?;
        let mut group = Vec::new();
        let has_device_fields = !(self.file.device_name.is_empty()
            && self.file.device_model.is_empty()
            && self.file.device_hardware.is_empty());
        if has_device_fields && self.devices.insert((device.writer.clone(), device.sensor.clone())) {
            let mut data = Map::new();
            data.insert("source_id".into(), json!(source_id));
            data.extend(device.fields.clone());
            data.insert("manufacturer".into(), json!(device.writer));
            group.push(GroupRecord {
                record_type: "source_device".into(),
                data,
                method: self.method(),
                key: match_key(&[&device.writer, sensor_part(&device.sensor), "source_device"]),
            });
        }
        Ok((source_id, group))
    }

    fn method(&self) -> String {
        format!("{}/v1", self.file.route)
    }

    fn finish_group(&mut self, store: &mut Store, label: &str, group: Vec<GroupRecord>) -> Result<(), String> {
        if !self.write || group.is_empty() {
            return Ok(());
        }
        for outcome in store.insert_group(&group)? {
            match outcome {
                GroupOutcome::Deleted(id) => self.left_out.push(format!("{label}: the stored record {id} is deleted")),
                other => {
                    let id = other.record_id().to_string();
                    if !self.record_ids.contains(&id) {
                        self.record_ids.push(id);
                    }
                }
            }
        }
        Ok(())
    }

    fn base(&self, source_id: &str) -> Map<String, Value> {
        let mut data = Map::new();
        data.insert("source_id".into(), json!(source_id));
        data.insert("received_at".into(), json!(self.received_at));
        data.insert("received_at_basis".into(), json!("import_time"));
        data
    }
}

fn sensor_part(sensor: &str) -> &str {
    if sensor.is_empty() { "-" } else { sensor }
}

/// The applied line for each of their fields: a matching `when-writer` line wins,
/// then the line without `when-writer`.
fn applied_lines<'a>(section: &'a Section, writer: &str) -> Vec<&'a MapLine> {
    let mut chosen: Vec<&MapLine> = Vec::new();
    for line in &section.lines {
        if chosen.iter().any(|picked| picked.their_field == line.their_field) {
            continue;
        }
        let candidates: Vec<&MapLine> = section.lines.iter().filter(|other| other.their_field == line.their_field).collect();
        let pick = candidates
            .iter()
            .find(|candidate| candidate.when_writer.as_ref().is_some_and(|writers| writers.iter().any(|name| name == writer)))
            .or_else(|| candidates.iter().find(|candidate| candidate.when_writer.is_none()));
        if let Some(pick) = pick {
            chosen.push(pick);
        }
    }
    section.lines.iter().filter(|line| chosen.iter().any(|picked| std::ptr::eq(*picked, *line))).collect()
}

fn source_value(converted: Value, our_unit: &str, input: &Value, their_unit: &str) -> Value {
    json!({
        "standard_value": converted,
        "standard_unit": our_unit,
        "input_value": input,
        "input_unit": their_unit
    })
}

fn positive_point(quantity: &str, value: &Value) -> bool {
    quantity == "temperature_deviation" || value.as_f64().is_some_and(|number| number > 0.0)
}

/// Apply a checked mapping to one input file.
pub fn apply_mapping(store: &mut Store, file: &MappingFile, input: &Value, write: bool) -> Result<Value, String> {
    let root = input.as_object().ok_or("the input must be a JSON object")?;
    refuse_personal_reports(root)?;
    let mut run = Run {
        file,
        received_at: crate::store::utc_now(),
        write,
        record_ids: Vec::new(),
        understood: BTreeSet::new(),
        preserved: BTreeSet::new(),
        left_out: Vec::new(),
        keys: HashSet::new(),
        devices: HashSet::new(),
    };
    let resolution = root
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("resolution"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut rows: Vec<(String, Value)> = Vec::new();
    let envelope = root.get("data").and_then(Value::as_array).filter(|_| root.contains_key("metadata"));
    if let Some(data) = envelope {
        if let Some(first) = data.first() {
            if is_timeseries_row(first) {
                if file.section("timeseries").is_some() {
                    rows.extend(data.iter().map(|row| ("timeseries".to_string(), row.clone())));
                } else {
                    run.left_out.push("timeseries: no mapping section".into());
                }
            } else {
                run.left_out.push(ENVELOPE_NOT_TIMESERIES.into());
            }
        }
    } else {
        for (key, value) in root {
            let Some(items) = value.as_array() else { continue };
            if items.is_empty() {
                continue;
            }
            if file.section(key).is_some() {
                rows.extend(items.iter().map(|row| (key.clone(), row.clone())));
            } else {
                run.left_out.push(format!("{key}: no mapping section"));
            }
        }
    }
    for (index, (section_name, row)) in rows.iter().enumerate() {
        let section = file.section(section_name).expect("rows only name mapped sections");
        let Some(object) = row.as_object() else {
            run.left_out.push(format!("{section_name} row {}: not an object", index + 1));
            continue;
        };
        if section_name == "timeseries" {
            map_sample(store, &mut run, section, object, resolution.as_deref())?;
        } else {
            map_session(store, &mut run, section, object)?;
        }
    }
    Ok(json!({
        "mapping": file.name,
        "record_ids": run.record_ids,
        "understood": run.understood.into_iter().collect::<Vec<_>>(),
        "preserved": run.preserved.into_iter().collect::<Vec<_>>(),
        "left_out": run.left_out,
    }))
}

fn refuse_personal_reports(root: &Map<String, Value>) -> Result<(), String> {
    let personal = |object: &Map<String, Value>| object.contains_key("report_type") || object.contains_key("reported_via");
    if personal(root) {
        return Err(REFUSE_PERSONAL_REPORT.into());
    }
    for value in root.values() {
        for row in value.as_array().into_iter().flatten() {
            if row.as_object().is_some_and(personal) {
                return Err(REFUSE_PERSONAL_REPORT.into());
            }
        }
    }
    Ok(())
}

fn is_timeseries_row(row: &Value) -> bool {
    row.as_object().is_some_and(|object| {
        ["timestamp", "type", "value", "unit"].iter().all(|key| object.contains_key(*key))
            && !object.contains_key("start_time")
    })
}

fn manual_entry(object: &Map<String, Value>) -> bool {
    object.get("entry_source").and_then(Value::as_str) == Some("manual")
}

fn map_session(store: &mut Store, run: &mut Run, section: &Section, row: &Map<String, Value>) -> Result<(), String> {
    let name = section.name.as_str();
    let id_field = section.setting("id_field").unwrap_or("id");
    let start_field = section.setting("start_field").unwrap_or("start_time");
    let end_field = section.setting("end_field").unwrap_or("end_time");
    let Some(id) = path_text(row, id_field) else {
        run.left_out.push(format!("{name}: missing {id_field}"));
        return Ok(());
    };
    let label = format!("{name} {id}");
    let Some(writer) = path_text(row, &run.file.writer_field) else {
        run.left_out.push(format!("{label}: missing {}", run.file.writer_field));
        return Ok(());
    };
    if manual_entry(row) {
        run.left_out.push(format!("{label}: entry_source manual is a personal report"));
        return Ok(());
    }
    let (Some(start_text), Some(end_text)) = (path_text(row, start_field), path_text(row, end_field)) else {
        let missing = if path_text(row, start_field).is_none() { start_field } else { end_field };
        run.left_out.push(format!("{label}: missing {missing}"));
        return Ok(());
    };
    let Some(start) = utc_text(&start_text) else {
        run.left_out.push(format!("{label}: {start_field} is not a timestamp with an offset"));
        return Ok(());
    };
    let Some(end) = utc_text(&end_text) else {
        run.left_out.push(format!("{label}: {end_field} is not a timestamp with an offset"));
        return Ok(());
    };
    if end < start {
        run.left_out.push(format!("{label}: end is before the start"));
        return Ok(());
    }
    let device = run.device(row, &writer);
    let session_key = match_key(&[&writer, sensor_part(&device.sensor), name, &id]);
    if !run.keys.insert(session_key.clone()) {
        run.left_out.push(format!("{label}: duplicate match key {session_key}"));
        return Ok(());
    }
    let (source_id, mut group) = run.source_group(store, &device)?;
    let mut supplied = row.clone();
    for path in run.used_paths().iter().chain([id_field.to_string(), start_field.to_string(), end_field.to_string()].iter()) {
        remove_path(&mut supplied, path);
    }
    let mut data = run.base(&source_id);
    data.insert("provider_record_id".into(), json!(id));
    data.insert("start_at".into(), json!(start));
    data.insert("end_at".into(), json!(end));
    data.insert("input_start_timestamp".into(), input_receipt(&start_text));
    data.insert("input_end_timestamp".into(), input_receipt(&end_text));
    let mut summaries = Vec::new();
    let mut stages = Map::new();
    let mut children = Vec::new();
    let mut written: HashSet<String> = HashSet::new();
    for line in applied_lines(section, &writer) {
        if line.when_absent && written.contains(&line.our_field) {
            continue;
        }
        let Some(value) = path_value(row, &line.their_field).cloned() else { continue };
        if value.is_null() {
            remove_path(&mut supplied, &line.their_field);
            continue;
        }
        let kind = target(name, &line.our_field).expect("checked mapping");
        if let Target::WorkoutText = kind {
            let Some(text) = value.as_str().filter(|text| !text.trim().is_empty()) else { continue };
            data.insert(line.our_field.clone(), json!(text));
        } else {
            let Some(number) = value.as_f64() else { continue };
            let (their_unit, our_unit) = line.units.clone().expect("checked mapping");
            let converted = convert_unit(number, &their_unit, &our_unit)?;
            match kind {
                Target::SleepField(_) => {
                    if number < 0.0 || (line.our_field == "interruptions" && value.as_u64().is_none()) {
                        continue;
                    }
                    if let Some(stage) = line.our_field.strip_prefix("stage_minutes.") {
                        stages.insert(stage.to_string(), converted);
                    } else {
                        data.insert(line.our_field.clone(), converted);
                    }
                }
                Target::WorkoutSummary(summary_type, _) => {
                    let mut summary = source_value(converted, &our_unit, &value, &their_unit);
                    summary["summary_type"] = json!(summary_type);
                    summaries.push(summary);
                }
                Target::Point(_) => {
                    if !positive_point(&line.our_field, &converted) {
                        continue;
                    }
                    let mut child = run.base(&source_id);
                    child.insert("quantity".into(), json!(line.our_field));
                    child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                    children.push(("point_measurement", "point", line.our_field.clone(), child));
                }
                Target::Summary(summary_type) => {
                    let mut child = run.base(&source_id);
                    child.insert("summary_type".into(), json!(summary_type));
                    child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                    children.push(("provider_summary", "summary", summary_type, child));
                }
                Target::Score(metric) => {
                    let mut child = run.base(&source_id);
                    child.insert("metric".into(), json!(metric));
                    child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                    child.insert("scale".into(), json!(line.scale.clone().expect("checked mapping")));
                    children.push(("provider_score", "score", metric, child));
                }
                Target::WorkoutText => unreachable!(),
            }
        }
        written.insert(line.our_field.clone());
        remove_path(&mut supplied, &line.their_field);
        run.understood.insert(line.their_field.clone());
    }
    if !stages.is_empty() {
        data.insert("stage_minutes".into(), Value::Object(stages));
    }
    data.insert("source_reported_summaries".into(), Value::Array(summaries));
    if !supplied.is_empty() {
        run.preserved.extend(supplied.keys().cloned());
        data.insert("supplied_fields".into(), Value::Object(supplied));
    }
    let record_type = if name == "sleep" { "sleep_session" } else { "workout_session" };
    group.push(GroupRecord {
        record_type: record_type.into(),
        data,
        method: run.method(),
        key: session_key.clone(),
    });
    for (record_type, kind, name_part, mut child) in children {
        child.insert("provider_record_id".into(), json!(id));
        child.insert("recorded_at".into(), json!(end));
        child.insert("input_timestamp".into(), input_receipt(&end_text));
        let key = format!("{session_key}:{}", match_key(&[kind, &name_part]));
        run.keys.insert(key.clone());
        group.push(GroupRecord { record_type: record_type.into(), data: child, method: run.method(), key });
    }
    run.finish_group(store, &label, group)
}

fn map_sample(
    store: &mut Store,
    run: &mut Run,
    section: &Section,
    row: &Map<String, Value>,
    resolution: Option<&str>,
) -> Result<(), String> {
    let time_field = section.setting("time_field").unwrap_or("timestamp");
    let value_field = section.setting("value_field").unwrap_or("value");
    let sample_type = row.get("type").and_then(Value::as_str).unwrap_or("").to_string();
    let time_text = path_text(row, time_field);
    let utc = time_text.as_deref().and_then(utc_text);
    let label = format!(
        "timeseries {} {}",
        if sample_type.is_empty() { "(no type)" } else { &sample_type },
        utc.clone().or_else(|| time_text.clone()).unwrap_or_else(|| "(no timestamp)".into())
    );
    let present = |key: &str| row.get(key).is_some_and(|value| !value.is_null());
    if present("period_end") {
        run.left_out.push(format!("{label}: aggregate row, not a raw sample"));
        return Ok(());
    }
    if let Some(resolution) = resolution.filter(|value| *value != "raw") {
        run.left_out.push(format!("{label}: resolution {resolution}: not raw samples"));
        return Ok(());
    }
    if resolution.is_none() && present("window_seconds") {
        run.left_out.push(format!("{label}: window_seconds present without resolution: cannot confirm raw"));
        return Ok(());
    }
    let (Some(time_text), Some(utc)) = (time_text, utc) else {
        run.left_out.push(format!("{label}: missing {time_field}"));
        return Ok(());
    };
    if sample_type.is_empty() {
        run.left_out.push(format!("{label}: missing type"));
        return Ok(());
    }
    let Some(writer) = path_text(row, &run.file.writer_field) else {
        run.left_out.push(format!("{label}: missing {}", run.file.writer_field));
        return Ok(());
    };
    if manual_entry(row) {
        run.left_out.push(format!("{label}: entry_source manual is a personal report"));
        return Ok(());
    }
    let their_field = format!("type.{sample_type}");
    let Some(line) = applied_lines(section, &writer).into_iter().find(|line| line.their_field == their_field) else {
        run.left_out.push(format!("{label}: unmapped type {sample_type}"));
        return Ok(());
    };
    let (their_unit, our_unit) = line.units.clone().expect("checked mapping");
    let sample_unit = row.get("unit").and_then(Value::as_str).unwrap_or("");
    if sample_unit != their_unit {
        let shown = if sample_unit.is_empty() { "(none)" } else { sample_unit };
        run.left_out.push(format!("{label}: unit {shown} does not match {their_unit}"));
        return Ok(());
    }
    let value = path_value(row, value_field).cloned().unwrap_or(Value::Null);
    let Some(number) = value.as_f64() else {
        run.left_out.push(format!("{label}: value is not a number"));
        return Ok(());
    };
    let converted = convert_unit(number, &their_unit, &our_unit)?;
    let kind = target("timeseries", &line.our_field).expect("checked mapping");
    if let Target::Point(_) = kind {
        if !positive_point(&line.our_field, &converted) {
            run.left_out.push(format!("{label}: {} must be positive", line.our_field));
            return Ok(());
        }
    }
    let device = run.device(row, &writer);
    let (record_type, kind_word, name_part) = match &kind {
        Target::Point(_) => ("point_measurement", "point", sample_type.clone()),
        Target::Summary(_) => ("provider_summary", "summary", sample_type.clone()),
        Target::Score(metric) => ("provider_score", "score", metric.clone()),
        _ => unreachable!("timeseries lines write points, summaries, or scores"),
    };
    let key = match_key(&[&writer, sensor_part(&device.sensor), "timeseries", kind_word, &name_part, &utc]);
    if !run.keys.insert(key.clone()) {
        run.left_out.push(format!("{label}: duplicate match key {key}"));
        return Ok(());
    }
    let (source_id, mut group) = run.source_group(store, &device)?;
    let mut supplied = row.clone();
    for path in run.used_paths().iter().chain([time_field.to_string(), value_field.to_string(), "type".into(), "unit".into()].iter()) {
        remove_path(&mut supplied, path);
    }
    let mut data = run.base(&source_id);
    match &kind {
        Target::Point(_) => {
            data.insert("quantity".into(), json!(line.our_field));
        }
        Target::Summary(summary_type) => {
            data.insert("summary_type".into(), json!(summary_type));
        }
        Target::Score(metric) => {
            data.insert("metric".into(), json!(metric));
            data.insert("scale".into(), json!(line.scale.clone().expect("checked mapping")));
        }
        _ => {}
    }
    data.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
    data.insert("recorded_at".into(), json!(utc));
    data.insert("input_timestamp".into(), input_receipt(&time_text));
    if !supplied.is_empty() {
        run.preserved.extend(supplied.keys().cloned());
        data.insert("supplied_fields".into(), Value::Object(supplied));
    }
    run.understood.insert(line.their_field.clone());
    group.push(GroupRecord { record_type: record_type.into(), data, method: run.method(), key });
    run.finish_group(store, &label, group)
}

#[cfg(test)]
mod tests {
    use super::{check_mapping, convert_unit, input_receipt, key_part, parse_mapping};

    const SMALL: &str = "# Small\n\n```ath-mapping\nname: small\nroute: small_route\nwriter_field: source.provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\nsleep_duration_seconds: asleep_seconds s s\n```\n";

    #[test]
    fn crlf_mapping_file_parses() {
        let lf = parse_mapping(SMALL).unwrap();
        let crlf = parse_mapping(&SMALL.replace('\n', "\r\n")).unwrap();
        assert_eq!(lf.name, "small");
        assert_eq!(crlf.name, "small");
        assert_eq!(crlf.route, lf.route);
        assert_eq!(crlf.sections[0].lines, lf.sections[0].lines);
        assert_eq!(crlf.sections[0].settings, lf.sections[0].settings);
        check_mapping(&crlf).unwrap();
    }

    #[test]
    fn one_hundred_seconds_is_the_binary64_minute() {
        let value = convert_unit(100.0, "s", "min").unwrap();
        assert_eq!(value.as_f64(), Some(1.6666666666666667));
        assert_eq!(serde_json::to_string(&value).unwrap(), "1.6666666666666667");
    }

    #[test]
    fn whole_seconds_are_an_integer_value() {
        let value = convert_unit(600.0, "s", "s").unwrap();
        assert_eq!(format!("{value:?}"), "Number(600)");
        assert_eq!(serde_json::to_string(&value).unwrap(), "600");
        let ratio = serde_json::to_string(&serde_json::json!(600.0 / 700.0)).unwrap();
        assert_eq!(ratio, "0.8571428571428571");
    }

    #[test]
    fn a_pair_outside_the_table_is_refused() {
        let text = SMALL.replace("asleep_seconds s s", "asleep_seconds ms s");
        let error = check_mapping(&parse_mapping(&text).unwrap()).unwrap_err();
        assert_eq!(error, "cannot convert ms to s for sleep_duration_seconds");
    }

    #[test]
    fn a_name_line_inside_a_section_is_a_mapping_line() {
        let text = "```ath-mapping\nname: n\nroute: r\nwriter_field: w\n[workout]\nname: name\n```";
        let file = parse_mapping(text).unwrap();
        assert_eq!(file.name, "n");
        assert_eq!(file.sections[0].lines[0].their_field, "name");
        assert_eq!(file.sections[0].lines[0].our_field, "name");
    }

    #[test]
    fn a_file_with_no_block_is_refused() {
        assert_eq!(parse_mapping("# nothing here").unwrap_err(), "the mapping file has no ath-mapping block");
    }

    #[test]
    fn receipts_keep_the_offset_text() {
        assert_eq!(input_receipt("2026-08-09T06:12:00.000Z")["timestamp_basis"], "source_instant");
        let offset = input_receipt("2026-08-09T10:00:00.000-04:00");
        assert_eq!(offset["utc_offset_text"], "-04:00");
        assert_eq!(offset["utc_offset_meaning"], "meaning_not_documented");
        assert_eq!(offset["timestamp_basis"], "source_offset_timestamp");
    }

    #[test]
    fn key_parts_escape_colons_and_backslashes() {
        assert_eq!(key_part("a:b\\c"), "a\\:b\\\\c");
    }
}
