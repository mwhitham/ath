//! Map a source whose reader already produced named fields.
//!
//! A mapping file is markdown. The program reads the first fenced block tagged
//! `ath-mapping`. Each line names their field and our field. A mapped field becomes
//! the typed value. Every other field stays in `supplied_fields` under its name.
//! Field names that belong to one source's layout come from its mapping file.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::{json, Map, Number, Value};

use crate::store::{GroupOutcome, GroupRecord, MappedSource, Store};

pub const REFUSE_PERSONAL_REPORT: &str = "refusing to turn a personal report into a measured record";

const SECTIONS: &[&str] = &["sleep", "workout", "timeseries"];
const SESSION_SETTINGS: &[&str] = &["id_field", "start_field", "end_field"];
const SAMPLE_SETTINGS: &[&str] = &["time_field", "type_field", "value_field", "unit_field"];
const SAMPLE_OPTIONAL_SETTINGS: &[&str] = &["aggregate_field", "window_field"];
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

#[derive(Debug, Clone, Default)]
pub struct MappingFile {
    pub name: String,
    pub route: String,
    pub writer_field: String,
    pub value_basis: String,
    pub sensor: Vec<String>,
    pub device_name: Vec<String>,
    pub device_model: Vec<String>,
    pub device_hardware: Vec<String>,
    /// `personal_when: field value`. A row whose field has that value is a personal report.
    pub personal_when: Option<(String, String)>,
    pub page: PageSettings,
    pub sections: Vec<Section>,
}

/// How a source wraps one page of rows: `page_rows`, `page_marker`, `page_section`,
/// and an optional `page_resolution` path whose value is `raw_resolution` for raw
/// samples (D193).
#[derive(Debug, Clone, Default)]
pub struct PageSettings {
    pub rows: Option<String>,
    pub marker: Option<String>,
    pub section: Option<String>,
    pub resolution: Option<String>,
    pub raw_resolution: Option<String>,
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

    /// A setting `check_mapping` requires.
    fn field(&self, key: &str) -> &str {
        self.setting(key).expect("checked mapping")
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
                    "value_basis" => file.value_basis = rest.to_string(),
                    "sensor" => file.sensor = list(),
                    "device_name" => file.device_name = list(),
                    "device_model" => file.device_model = list(),
                    "device_hardware" => file.device_hardware = list(),
                    "personal_when" => {
                        let parts: Vec<&str> = rest.split_whitespace().collect();
                        let [field, value] = parts.as_slice() else {
                            return Err(format!("could not read '{line}'"));
                        };
                        file.personal_when = Some((field.to_string(), value.to_string()));
                    }
                    "page_rows" => file.page.rows = Some(rest.to_string()),
                    "page_marker" => file.page.marker = Some(rest.to_string()),
                    "page_section" => file.page.section = Some(rest.to_string()),
                    "page_resolution" => file.page.resolution = Some(rest.to_string()),
                    "raw_resolution" => file.page.raw_resolution = Some(rest.to_string()),
                    _ => return Err(format!("could not read '{line}'")),
                }
            }
            Some(section) => {
                if SESSION_SETTINGS.contains(&key) || SAMPLE_SETTINGS.contains(&key) || SAMPLE_OPTIONAL_SETTINGS.contains(&key) {
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

/// Their name for a value. A timeseries line reads `<type_field>.<type>`, and the
/// type is the name.
fn statistic_name<'a>(section: &Section, their_field: &'a str) -> &'a str {
    if section.name != "timeseries" {
        return their_field;
    }
    match section.setting("type_field") {
        Some(type_field) => their_field
            .strip_prefix(type_field)
            .and_then(|rest| rest.strip_prefix('.'))
            .unwrap_or(their_field),
        None => their_field.split_once('.').map_or(their_field, |(_, rest)| rest),
    }
}

/// The heart-rate variability statistic a target claims to store, from its name (D194).
fn claimed_statistic(our_field: &str) -> Option<&'static str> {
    let name = our_field
        .strip_prefix("provider_summary.")
        .or_else(|| our_field.strip_prefix("provider_score."))
        .unwrap_or(our_field)
        .to_ascii_lowercase();
    if name.contains("rmssd") {
        Some("rmssd")
    } else if name.contains("sdnn") {
        Some("sdnn")
    } else {
        None
    }
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
    for section in &file.sections {
        for line in &section.lines {
            refuse_line(section, line)?;
        }
    }
    for (key, value) in [
        ("name", &file.name),
        ("route", &file.route),
        ("writer_field", &file.writer_field),
        ("value_basis", &file.value_basis),
    ] {
        if value.is_empty() {
            return Err(format!("the mapping file needs {key}"));
        }
    }
    if !FILE_BASES.contains(&file.value_basis.as_str()) {
        return Err(format!(
            "value_basis {} is not one of {}",
            file.value_basis,
            FILE_BASES.join(", ")
        ));
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
        check_settings(section)?;
    }
    check_page(file)
}

/// Each section names the fields that hold its rows. Nothing is assumed (D193).
fn check_settings(section: &Section) -> Result<(), String> {
    let (required, optional): (&[&str], &[&str]) = if section.name == "timeseries" {
        (SAMPLE_SETTINGS, SAMPLE_OPTIONAL_SETTINGS)
    } else {
        (SESSION_SETTINGS, &[])
    };
    for key in required {
        if section.setting(key).is_none() {
            return Err(format!("[{}] needs {key}", section.name));
        }
    }
    for key in section.settings.keys() {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(format!("{key} is not a setting of [{}]", section.name));
        }
    }
    if section.name == "timeseries" {
        let type_field = section.field("type_field");
        for line in &section.lines {
            if statistic_name(section, &line.their_field) == line.their_field.as_str() {
                return Err(format!(
                    "a [timeseries] line reads {type_field}.<type>, not {}",
                    line.their_field
                ));
            }
        }
    }
    Ok(())
}

fn check_page(file: &MappingFile) -> Result<(), String> {
    let page = &file.page;
    let described = page.rows.is_some()
        || page.marker.is_some()
        || page.section.is_some()
        || page.resolution.is_some()
        || page.raw_resolution.is_some();
    if !described {
        return Ok(());
    }
    for (key, value) in [("page_rows", &page.rows), ("page_marker", &page.marker), ("page_section", &page.section)] {
        if value.is_none() {
            return Err(format!("the mapping file needs {key} when it describes a page"));
        }
    }
    let section = page.section.as_deref().unwrap_or_default();
    if file.section(section).is_none() {
        return Err(format!("page_section {section} is not a section of this file"));
    }
    if page.resolution.is_some() != page.raw_resolution.is_some() {
        return Err("page_resolution and raw_resolution go together".into());
    }
    Ok(())
}

/// The four refusals in D179 that come from a single line.
fn refuse_line(section: &Section, line: &MapLine) -> Result<(), String> {
    let our = line.our_field.as_str();
    let name = statistic_name(section, &line.their_field);
    if let Some(statistic) = claimed_statistic(our) {
        let lower = name.to_ascii_lowercase();
        let names_rmssd = lower.contains("rmssd");
        let names_sdnn = lower.contains("sdnn");
        let named = if statistic == "rmssd" {
            (names_rmssd && !names_sdnn)
                || (!names_rmssd
                    && !names_sdnn
                    && line.when_writer.is_some()
                    && line.basis.as_deref().is_some_and(|basis| RMSSD_BASES.contains(&basis)))
        } else {
            names_sdnn && !names_rmssd
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
        let known = if our == "activity_type" {
            ACTIVITY_TYPE_BASES.contains(&basis.as_str())
        } else if our == "hrv_rmssd" || our == "hrv_sdnn" || heart_rate_summary {
            VALUE_BASES.contains(&basis.as_str())
        } else {
            return Err(
                "basis is only used for activity_type, hrv_rmssd, hrv_sdnn, and heart-rate summaries".into(),
            );
        };
        if !known {
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
        if our == "activity_type" && line.basis.is_none() {
            return Err(format!("activity_type needs a basis: one of {}", ACTIVITY_TYPE_BASES.join(", ")));
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

/// Every `field_basis.value` in the record format (D185, D187, D192). A test keeps
/// this list equal to `ValueBasis` in `src/v1/schema.ts`.
pub const VALUE_BASES: &[&str] = &[
    "provider_or_aggregator",
    "provider",
    "hrv_mean_rmssd",
    "provider_documented_rmssd",
    "calculated_on_import",
    "entered_by_hand",
];

/// Bases that say the value is RMSSD.
const RMSSD_BASES: &[&str] = &["hrv_mean_rmssd", "provider_documented_rmssd"];

/// `value_basis` values a mapping file may give every value it writes (D187, D192).
pub const FILE_BASES: &[&str] = &["provider", "provider_or_aggregator"];

/// `field_basis.activity_type` values an `activity_type` line may name with `basis` (D185, D192).
pub const ACTIVITY_TYPE_BASES: &[&str] = &["aggregator_normalized", "provider_label"];

fn value_basis(value: &str) -> Value {
    json!({ "value": value })
}

/// A line's own basis, or else the basis the file gives every value it writes.
fn line_basis<'a>(line: &'a MapLine, file: &'a MappingFile) -> &'a str {
    line.basis.as_deref().unwrap_or(&file.value_basis)
}

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
    record_ids: Vec<String>,
    new: usize,
    changed: usize,
    unchanged: usize,
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

    /// A row whose `personal_when` field has the named value is a personal report.
    fn personal_report(&self, row: &Map<String, Value>) -> Option<String> {
        let (field, value) = self.file.personal_when.as_ref()?;
        (path_text(row, field).as_deref() == Some(value.as_str())).then(|| format!("{field} {value} is a personal report"))
    }

    /// The source, and the device's `source_device` record the first time it is seen.
    /// None when a person deleted the source: its rows are left out, not written again.
    fn source_group(
        &mut self,
        store: &mut Store,
        device: &Device,
        label: &str,
    ) -> Result<Option<(String, Vec<GroupRecord>)>, String> {
        let source_id = match store.ensure_mapped_source(&device.writer, &self.file.route, &device.writer, &device.sensor)? {
            MappedSource::Current(id) => id,
            MappedSource::Deleted(id) => {
                self.left_out.push(format!("{label}: its source {id} is deleted"));
                return Ok(None);
            }
        };
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
        Ok(Some((source_id, group)))
    }

    fn method(&self) -> String {
        format!("{}/v1", self.file.route)
    }

    fn finish_group(&mut self, store: &mut Store, label: &str, group: Vec<GroupRecord>) -> Result<(), String> {
        if group.is_empty() {
            return Ok(());
        }
        for outcome in store.insert_group(&group)? {
            match &outcome {
                GroupOutcome::Deleted(id) => {
                    self.left_out.push(format!("{label}: the stored record {id} is deleted"));
                    continue;
                }
                GroupOutcome::Inserted(_) => self.new += 1,
                GroupOutcome::Revised(_) => self.changed += 1,
                GroupOutcome::Unchanged(_) => self.unchanged += 1,
            }
            let id = outcome.record_id().to_string();
            if !self.record_ids.contains(&id) {
                self.record_ids.push(id);
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

/// Apply a checked mapping to one input file, in one transaction (D189).
///
/// A preview runs exactly the same writes and rolls them back, so it reports what
/// the import would do. Any error rolls back everything, so nothing is half written.
pub fn apply_mapping(store: &mut Store, file: &MappingFile, input: &Value, write: bool) -> Result<Value, String> {
    let root = input.as_object().ok_or("the input must be a JSON object")?;
    refuse_personal_reports(root)?;
    store.begin()?;
    let mut result = match map_input(store, file, root) {
        Ok(result) => result,
        Err(error) => {
            let _ = store.finish(false);
            return Err(error);
        }
    };
    store.finish(write)?;
    if !write {
        result["record_ids"] = json!([]);
    }
    Ok(result)
}

fn map_input(store: &mut Store, file: &MappingFile, root: &Map<String, Value>) -> Result<Value, String> {
    let mut run = Run {
        file,
        received_at: crate::store::utc_now(),
        record_ids: Vec::new(),
        new: 0,
        changed: 0,
        unchanged: 0,
        understood: BTreeSet::new(),
        preserved: BTreeSet::new(),
        left_out: Vec::new(),
        keys: HashSet::new(),
        devices: HashSet::new(),
    };
    let resolution = file.page.resolution.as_deref().and_then(|path| path_text(root, path));
    let mut rows: Vec<(String, Value)> = Vec::new();
    match page_rows(file, root) {
        Some((section_name, data)) => {
            if let Some(first) = data.first() {
                if row_fits(file, section_name, first) {
                    rows.extend(data.iter().map(|row| (section_name.to_string(), row.clone())));
                } else {
                    run.left_out.push(format!(
                        "envelope is not a {section_name} page: pass its {} array under workout or sleep",
                        file.page.rows.as_deref().unwrap_or_default()
                    ));
                }
            }
        }
        None => {
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
        "outcome": { "new": run.new, "changed": run.changed, "unchanged": run.unchanged },
        "understood": run.understood.into_iter().collect::<Vec<_>>(),
        "preserved": run.preserved.into_iter().collect::<Vec<_>>(),
        "left_out": run.left_out,
    }))
}

/// The rows of a page, and the section they map with, when the input is a page.
fn page_rows<'a>(file: &'a MappingFile, root: &'a Map<String, Value>) -> Option<(&'a str, &'a Vec<Value>)> {
    let page = &file.page;
    let rows = root.get(page.rows.as_deref()?)?.as_array()?;
    root.contains_key(page.marker.as_deref()?).then_some(())?;
    Some((page.section.as_deref()?, rows))
}

/// Whether a row has the fields its section reads. A sample row also has none of
/// the start fields the file's session sections read.
fn row_fits(file: &MappingFile, section_name: &str, row: &Value) -> bool {
    let (Some(object), Some(section)) = (row.as_object(), file.section(section_name)) else {
        return false;
    };
    let keys: &[&str] = if section.name == "timeseries" { SAMPLE_SETTINGS } else { SESSION_SETTINGS };
    if !keys.iter().all(|key| path_value(object, section.field(key)).is_some()) {
        return false;
    }
    section.name != "timeseries"
        || file
            .sections
            .iter()
            .filter(|other| other.name != "timeseries")
            .filter_map(|other| other.setting("start_field"))
            .all(|start| path_value(object, start).is_none())
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

fn map_session(store: &mut Store, run: &mut Run, section: &Section, row: &Map<String, Value>) -> Result<(), String> {
    let name = section.name.as_str();
    let id_field = section.field("id_field");
    let start_field = section.field("start_field");
    let end_field = section.field("end_field");
    let Some(id) = path_text(row, id_field) else {
        run.left_out.push(format!("{name}: missing {id_field}"));
        return Ok(());
    };
    let label = format!("{name} {id}");
    let Some(writer) = path_text(row, &run.file.writer_field) else {
        run.left_out.push(format!("{label}: missing {}", run.file.writer_field));
        return Ok(());
    };
    if let Some(reason) = run.personal_report(row) {
        run.left_out.push(format!("{label}: {reason}"));
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
    let Some((source_id, mut group)) = run.source_group(store, &device, &label)? else {
        return Ok(());
    };
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
    let mut field_basis = Map::new();
    field_basis.insert("value".into(), json!(run.file.value_basis));
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
            if let Some(basis) = line.basis.as_ref().filter(|_| line.our_field == "activity_type") {
                field_basis.insert("activity_type".into(), json!(basis));
            }
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
                    child.insert("field_basis".into(), value_basis(line_basis(line, run.file)));
                    children.push(("point_measurement", "point", line.our_field.clone(), child));
                }
                Target::Summary(summary_type) => {
                    let mut child = run.base(&source_id);
                    child.insert("summary_type".into(), json!(summary_type));
                    child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                    child.insert("field_basis".into(), value_basis(line_basis(line, run.file)));
                    children.push(("provider_summary", "summary", summary_type, child));
                }
                Target::Score(metric) => {
                    let mut child = run.base(&source_id);
                    child.insert("metric".into(), json!(metric));
                    child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                    child.insert("scale".into(), json!(line.scale.clone().expect("checked mapping")));
                    child.insert("field_basis".into(), value_basis("provider"));
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
    data.insert("field_basis".into(), Value::Object(field_basis));
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
    let time_field = section.field("time_field");
    let type_field = section.field("type_field");
    let value_field = section.field("value_field");
    let unit_field = section.field("unit_field");
    let sample_type = path_text(row, type_field).unwrap_or_default();
    let time_text = path_text(row, time_field);
    let utc = time_text.as_deref().and_then(utc_text);
    let label = format!(
        "timeseries {} {}",
        if sample_type.is_empty() { "(no type)" } else { &sample_type },
        utc.clone().or_else(|| time_text.clone()).unwrap_or_else(|| "(no timestamp)".into())
    );
    let present = |key: &str| path_value(row, key).is_some_and(|value| !value.is_null());
    if section.setting("aggregate_field").is_some_and(|field| present(field)) {
        run.left_out.push(format!("{label}: aggregate row, not a raw sample"));
        return Ok(());
    }
    let raw = run.file.page.raw_resolution.as_deref();
    if let Some(resolution) = resolution.filter(|value| Some(*value) != raw) {
        run.left_out.push(format!("{label}: resolution {resolution}: not raw samples"));
        return Ok(());
    }
    if let Some(window) = section.setting("window_field").filter(|field| resolution.is_none() && present(field)) {
        run.left_out.push(format!("{label}: {window} present without resolution: cannot confirm raw"));
        return Ok(());
    }
    let (Some(time_text), Some(utc)) = (time_text, utc) else {
        run.left_out.push(format!("{label}: missing {time_field}"));
        return Ok(());
    };
    if sample_type.is_empty() {
        run.left_out.push(format!("{label}: missing {type_field}"));
        return Ok(());
    }
    let Some(writer) = path_text(row, &run.file.writer_field) else {
        run.left_out.push(format!("{label}: missing {}", run.file.writer_field));
        return Ok(());
    };
    if let Some(reason) = run.personal_report(row) {
        run.left_out.push(format!("{label}: {reason}"));
        return Ok(());
    }
    let their_field = format!("{type_field}.{sample_type}");
    let Some(line) = applied_lines(section, &writer).into_iter().find(|line| line.their_field == their_field) else {
        run.left_out.push(format!("{label}: unmapped type {sample_type}"));
        return Ok(());
    };
    let (their_unit, our_unit) = line.units.clone().expect("checked mapping");
    let sample_unit = path_text(row, unit_field).unwrap_or_default();
    if sample_unit != their_unit {
        let shown = if sample_unit.is_empty() { "(none)" } else { &sample_unit };
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
    let Some((source_id, mut group)) = run.source_group(store, &device, &label)? else {
        return Ok(());
    };
    let mut supplied = row.clone();
    for path in run.used_paths().iter().chain([time_field, value_field, type_field, unit_field].map(str::to_string).iter()) {
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
    if matches!(kind, Target::Score(_)) {
        data.insert("field_basis".into(), value_basis("provider"));
    } else {
        data.insert("field_basis".into(), value_basis(line_basis(line, run.file)));
    }
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

    const SMALL: &str = "# Small\n\n```ath-mapping\nname: small\nroute: small_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\nsleep_duration_seconds: asleep_seconds s s\n```\n";

    const SAMPLES: &str = "```ath-mapping\nname: samples\nroute: samples_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[timeseries]\ntime_field: at\ntype_field: kind\nvalue_field: reading\nunit_field: unit\nkind.rmssd_night: hrv_rmssd ms ms\n```";

    fn refusal(text: &str) -> String {
        check_mapping(&parse_mapping(text).unwrap()).unwrap_err()
    }

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
        assert_eq!(refusal(&text), "cannot convert ms to s for sleep_duration_seconds");
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

    #[test]
    fn every_section_names_the_fields_of_its_rows() {
        assert_eq!(refusal(&SMALL.replace("id_field: id\n", "")), "[sleep] needs id_field");
        assert_eq!(refusal(&SAMPLES.replace("unit_field: unit\n", "")), "[timeseries] needs unit_field");
        assert_eq!(refusal(&SMALL.replace("id_field: id\n", "id_field: id\ntime_field: at\n")), "time_field is not a setting of [sleep]");
        check_mapping(&parse_mapping(SAMPLES).unwrap()).unwrap();
    }

    #[test]
    fn a_timeseries_line_reads_the_type_field() {
        let text = SAMPLES.replace("kind.rmssd_night:", "rmssd_night:");
        assert_eq!(refusal(&text), "a [timeseries] line reads kind.<type>, not rmssd_night");
    }

    #[test]
    fn a_page_is_described_in_full() {
        let page = SAMPLES.replace("value_basis: provider\n", "value_basis: provider\npage_rows: rows\npage_section: timeseries\n");
        assert_eq!(refusal(&page), "the mapping file needs page_marker when it describes a page");
        let elsewhere = page.replace("page_section: timeseries\n", "page_marker: meta\npage_section: sleep\n");
        assert_eq!(refusal(&elsewhere), "page_section sleep is not a section of this file");
        let half = page.replace("page_section: timeseries\n", "page_marker: meta\npage_section: timeseries\npage_resolution: meta.resolution\n");
        assert_eq!(refusal(&half), "page_resolution and raw_resolution go together");
        let full = half.replace("page_resolution: meta.resolution\n", "page_resolution: meta.resolution\nraw_resolution: raw\n");
        check_mapping(&parse_mapping(&full).unwrap()).unwrap();
    }

    #[test]
    fn personal_when_names_a_field_and_a_value() {
        let text = SMALL.replace("value_basis: provider\n", "value_basis: provider\npersonal_when: origin typed\n");
        let file = parse_mapping(&text).unwrap();
        assert_eq!(file.personal_when, Some(("origin".to_string(), "typed".to_string())));
        let broken = SMALL.replace("value_basis: provider\n", "value_basis: provider\npersonal_when: origin\n");
        assert_eq!(parse_mapping(&broken).unwrap_err(), "could not read 'personal_when: origin'");
    }

    #[test]
    fn an_hrv_value_names_its_statistic_in_any_record() {
        assert_eq!(
            refusal(&SMALL.replace("sleep_duration_seconds: asleep_seconds s s", "nightly_hrv: provider_summary.hrv_rmssd ms ms")),
            "refusing to map nightly_hrv onto provider_summary.hrv_rmssd; the field does not name the statistic"
        );
        assert_eq!(
            refusal(&SMALL.replace("sleep_duration_seconds: asleep_seconds s s", "hrv_rmssd_night: provider_score.sdnn_balance score score scale 0-100")),
            "refusing to map hrv_rmssd_night onto provider_score.sdnn_balance; the field does not name the statistic"
        );
        assert_eq!(
            refusal(&SAMPLES.replace("kind.rmssd_night:", "kind.sdnn_night:")),
            "refusing to map sdnn_night onto hrv_rmssd; the field does not name the statistic"
        );
        let summary = SMALL.replace("sleep_duration_seconds: asleep_seconds s s", "rmssd_night: provider_summary.hrv_rmssd ms ms");
        check_mapping(&parse_mapping(&summary).unwrap()).unwrap();
    }

    #[test]
    fn a_field_without_the_statistic_needs_an_rmssd_basis_and_writers() {
        let line = |rest: &str| SMALL.replace("sleep_duration_seconds: asleep_seconds s s", &format!("nightly: hrv_rmssd ms ms{rest}"));
        let refused = "refusing to map nightly onto hrv_rmssd; the field does not name the statistic";
        assert_eq!(refusal(&line("")), refused);
        assert_eq!(refusal(&line(" basis hrv_mean_rmssd")), refused);
        assert_eq!(refusal(&line(" basis provider when-writer a")), refused);
        check_mapping(&parse_mapping(&line(" basis hrv_mean_rmssd when-writer a,b")).unwrap()).unwrap();
    }
}
