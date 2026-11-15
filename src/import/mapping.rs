//! Mapped imports: `ath import <file.json> --mapping <name>`.
//!
//! A mapping file is Markdown. The program reads its first fenced block
//! tagged `ath-mapping`. A trailing carriage return on a line is removed, so a
//! file saved with Windows line endings reads the same. The block names the
//! source's layout and, for each of their fields, our field:
//!
//! - File settings: `name`, `route`, `writer_field`, `value_basis`, the device
//!   fields (`sensor`, `device_name`, `device_model`, `device_hardware`),
//!   `personal_when`, and the page settings (`page_rows`, `page_marker`,
//!   `page_section`, `page_resolution`, `raw_resolution`) (D183, D193).
//! - Sections `[sleep]`, `[workout]`, and `[timeseries]`, each naming the
//!   fields of its rows, then one line per mapped field:
//!   `their_field: our_field [their_unit our_unit] [basis B] [scale S]
//!   [when-absent] [when-writer a,b]`.
//!
//! Units convert only by the fixed pairs of `convert_unit` (D180). For each of
//! their fields, a line whose `when-writer` names the row's writer wins over
//! the line without one. The first line that writes our field wins, and
//! `when-absent` lines come last (D196). Every field no line used stays in
//! `supplied_fields` under the name it arrived with.
//!
//! Four semantic refusals hold for every mapping file (D179, D194):
//!
//! 1. A personal report never becomes a measured record.
//! 2. A heart-rate variability field becomes RMSSD or SDNN only when its own
//!    name says which statistic it is, or, for RMSSD, when a writer-specific
//!    line claims an RMSSD basis.
//! 3. Sleep efficiency is calculated from asleep and in-bed time, never mapped.
//! 4. A provider's peak heart rate never becomes the athlete's known maximum.
//!
//! The file is parsed and checked into typed settings before anything is read
//! or written. A mapping file is data: nothing in it runs as code.
//!
//! These are the rules of `crates/ath-core/src/mapping.rs` in 2.1.0.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::input_policy::MAX_MAPPED_INPUT_BYTES;
use crate::json::{Map, Number, Value};
use crate::model::{point_measurement_unit, RecordType};

use super::provider::{CandidateSink, KeyedPolicy, MatchPolicy, ReportKind, Route, RouteReader};
use super::shared::{format_datetime, keyed_candidate};

pub const REFUSE_PERSONAL_REPORT: &str = "refusing to turn a personal report into a measured record";

/// The policy of mapped imports: a changed record replaces the stored object
/// and keeps its first `received_at` (D182).
pub const MAPPED_POLICY: MatchPolicy = MatchPolicy::Keyed(KeyedPolicy {
    merge: false,
    keep_received_at: true,
    correction_basis: crate::model::BecameKnownAtBasis::ProviderImport,
});

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

/// Every `field_basis.value` in the record format (D185, D187, D192).
pub const VALUE_BASES: &[&str] = &[
    "provider_or_aggregator",
    "provider",
    "hrv_mean_rmssd",
    "provider_documented_rmssd",
    "calculated_on_import",
    "entered_by_hand",
];

/// The bases a mapping line may claim with `basis` (D195). A mapping does not
/// calculate, and its rows are not entered by hand.
pub const LINE_BASES: &[&str] = &["provider", "provider_or_aggregator", "hrv_mean_rmssd", "provider_documented_rmssd"];

/// Bases that say the value is RMSSD. Only an `hrv_rmssd` line may claim them.
const RMSSD_BASES: &[&str] = &["hrv_mean_rmssd", "provider_documented_rmssd"];

/// `value_basis` values a mapping file may give every value it writes (D187, D192).
pub const FILE_BASES: &[&str] = &["provider", "provider_or_aggregator"];

/// `field_basis.activity_type` values an `activity_type` line may name (D185, D192).
pub const ACTIVITY_TYPE_BASES: &[&str] = &["aggregator_normalized", "provider_label"];

// ---- The file as written ----------------------------------------------------

/// A mapping file as written, before it is checked.
#[derive(Debug, Clone, Default, PartialEq)]
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

/// How a source wraps one page of rows (D193).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageSettings {
    pub rows: Option<String>,
    pub marker: Option<String>,
    pub section: Option<String>,
    pub resolution: Option<String>,
    pub raw_resolution: Option<String>,
}

/// One section as written: its settings in the order they appear, and its lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Section {
    pub name: String,
    pub settings: Vec<(String, String)>,
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
        self.settings.iter().rev().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
    }

    fn set(&mut self, key: &str, value: &str) {
        match self.settings.iter_mut().find(|(name, _)| name == key) {
            Some(existing) => existing.1 = value.to_string(),
            None => self.settings.push((key.to_string(), value.to_string())),
        }
    }
}

/// Read the first `ath-mapping` block. A trailing `\r` on a line is removed.
pub fn parse_mapping(text: &str) -> Result<MappingFile> {
    parse(text).map_err(Error::refused)
}

fn parse(text: &str) -> std::result::Result<MappingFile, String> {
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
        let unreadable = || format!("could not read '{line}'");
        if line.starts_with('[') {
            let name = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or_else(unreadable)?;
            if let Some(section) = current.take() {
                file.sections.push(section);
            }
            current = Some(Section { name: name.to_string(), ..Section::default() });
            continue;
        }
        let (key, rest) = line.split_once(':').ok_or_else(unreadable)?;
        let key = key.trim();
        let rest = rest.trim();
        if key.is_empty() || rest.is_empty() {
            return Err(unreadable());
        }
        match current.as_mut() {
            None => {
                let list =
                    || rest.split(',').map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect();
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
                            return Err(unreadable());
                        };
                        file.personal_when = Some((field.to_string(), value.to_string()));
                    }
                    "page_rows" => file.page.rows = Some(rest.to_string()),
                    "page_marker" => file.page.marker = Some(rest.to_string()),
                    "page_section" => file.page.section = Some(rest.to_string()),
                    "page_resolution" => file.page.resolution = Some(rest.to_string()),
                    "raw_resolution" => file.page.raw_resolution = Some(rest.to_string()),
                    _ => return Err(unreadable()),
                }
            }
            Some(section) => {
                if SESSION_SETTINGS.contains(&key)
                    || SAMPLE_SETTINGS.contains(&key)
                    || SAMPLE_OPTIONAL_SETTINGS.contains(&key)
                {
                    section.set(key, rest);
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

fn parse_line(line: &str, their_field: &str, rest: &str) -> std::result::Result<MapLine, String> {
    let unreadable = || format!("could not read '{line}'");
    let mut tokens = rest.split_whitespace();
    let our_field = tokens.next().ok_or_else(unreadable)?;
    let mut parsed =
        MapLine { their_field: their_field.to_string(), our_field: our_field.to_string(), ..MapLine::default() };
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
                    _ => parsed.when_writer.replace(value.split(',').map(str::to_string).collect()).is_some(),
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
    if parsed.when_writer.as_ref().is_some_and(|writers| writers.iter().any(|writer| writer.trim().is_empty())) {
        return Err(unreadable());
    }
    Ok(parsed)
}

// ---- The checked mapping -----------------------------------------------------

/// What one mapping line writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A sleep session field in its standard unit.
    SleepField(&'static str),
    /// A workout's activity type, name, description, or result text.
    WorkoutText,
    /// A workout's source-reported summary, by summary type, in its unit.
    WorkoutSummary(String, &'static str),
    /// A point measurement in its standard unit.
    Point(&'static str),
    /// A provider summary, by summary type.
    Summary(String),
    /// A provider score, by metric.
    Score(String),
}

fn target(section: &str, our_field: &str) -> Option<Target> {
    if let Some(summary) = our_field.strip_prefix("provider_summary.") {
        return (!summary.is_empty()).then(|| Target::Summary(summary.to_string()));
    }
    if let Some(metric) = our_field.strip_prefix("provider_score.") {
        return (!metric.is_empty()).then(|| Target::Score(metric.to_string()));
    }
    if let Some(unit) = point_measurement_unit(our_field) {
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

/// A checked mapping line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub their_field: String,
    pub our_field: String,
    pub target: Target,
    /// Their unit and our unit. Every target except workout text has one.
    pub units: Option<(String, String)>,
    pub basis: Option<String>,
    /// A provider score's scale.
    pub scale: Option<String>,
    pub when_absent: bool,
    pub when_writer: Option<Vec<String>>,
}

/// The fields that hold a section's rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    Session {
        id_field: String,
        start_field: String,
        end_field: String,
    },
    Samples {
        time_field: String,
        type_field: String,
        value_field: String,
        unit_field: String,
        aggregate_field: Option<String>,
        window_field: Option<String>,
    },
}

/// One checked section: `sleep`, `workout`, or `timeseries`.
#[derive(Debug, Clone, PartialEq)]
pub struct MappedSection {
    pub name: String,
    pub layout: Layout,
    pub lines: Vec<Line>,
}

/// How a page of rows is wrapped (D193).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub rows: String,
    pub marker: String,
    pub section: String,
    /// The resolution field and the value that means raw samples.
    pub resolution: Option<(String, String)>,
}

/// A mapping that passed every check. Nothing is read or written before this exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    pub name: String,
    pub route: String,
    pub writer_field: String,
    pub value_basis: String,
    pub sensor: Vec<String>,
    pub device_name: Vec<String>,
    pub device_model: Vec<String>,
    pub device_hardware: Vec<String>,
    pub personal_when: Option<(String, String)>,
    pub page: Option<Page>,
    pub sections: Vec<MappedSection>,
}

impl Mapping {
    fn section(&self, name: &str) -> Option<&MappedSection> {
        self.sections.iter().find(|section| section.name == name)
    }

    /// The match key method: `<route>/v1`.
    pub fn method(&self) -> String {
        format!("{}/v1", self.route)
    }
}

/// Parse and check a mapping file.
pub fn load_mapping(text: &str) -> Result<Mapping> {
    check_mapping(&parse_mapping(text)?)
}

/// Check a mapping before anything is read or written. The first problem
/// found is returned, in the 2.1.0 order.
pub fn check_mapping(file: &MappingFile) -> Result<Mapping> {
    check(file).map_err(Error::refused)
}

fn check(file: &MappingFile) -> std::result::Result<Mapping, String> {
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
        return Err(format!("value_basis {} is not one of {}", file.value_basis, FILE_BASES.join(", ")));
    }
    if BUILT_IN_ROUTES.contains(&file.route.as_str()) {
        return Err(format!("route {} is already used by a built-in importer", file.route));
    }
    let method = format!("{}/v1", file.route);
    if BUILT_IN_METHODS.contains(&method.as_str()) {
        return Err(format!("match key method {method} is already used by a built-in importer"));
    }
    let mut sections = Vec::new();
    for section in &file.sections {
        if !SECTIONS.contains(&section.name.as_str()) {
            return Err(format!("unknown mapping section {}", section.name));
        }
        let mut when_absent = HashSet::new();
        let mut fallbacks = HashSet::new();
        let mut lines = Vec::new();
        for line in &section.lines {
            lines.push(check_line(&section.name, line)?);
            if line.when_absent && !when_absent.insert(line.our_field.clone()) {
                return Err(format!("two when-absent lines write {}", line.our_field));
            }
            if line.when_writer.is_none() && !fallbacks.insert(line.their_field.clone()) {
                return Err(format!("two lines read {} without when-writer", line.their_field));
            }
        }
        let layout = check_settings(section)?;
        sections.push(MappedSection { name: section.name.clone(), layout, lines });
    }
    let page = check_page(file)?;
    Ok(Mapping {
        name: file.name.clone(),
        route: file.route.clone(),
        writer_field: file.writer_field.clone(),
        value_basis: file.value_basis.clone(),
        sensor: file.sensor.clone(),
        device_name: file.device_name.clone(),
        device_model: file.device_model.clone(),
        device_hardware: file.device_hardware.clone(),
        personal_when: file.personal_when.clone(),
        page,
        sections,
    })
}

/// Their name for a value. A timeseries line reads `<type_field>.<type>`, and
/// the type is the name.
fn statistic_name<'a>(section: &Section, their_field: &'a str) -> &'a str {
    if section.name != "timeseries" {
        return their_field;
    }
    match section.setting("type_field") {
        Some(type_field) => {
            their_field.strip_prefix(type_field).and_then(|rest| rest.strip_prefix('.')).unwrap_or(their_field)
        }
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

/// The refusals in D179 that come from a single line.
fn refuse_line(section: &Section, line: &MapLine) -> std::result::Result<(), String> {
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
            return Err(format!("refusing to map {name} onto {our}; the field does not name the statistic"));
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

fn check_line(section: &str, line: &MapLine) -> std::result::Result<Line, String> {
    let our = line.our_field.as_str();
    let Some(kind) = target(section, our) else {
        return Err(format!("unknown field {our}"));
    };
    if let Some(basis) = &line.basis {
        let heart_rate_summary =
            matches!(kind, Target::Summary(_)) && line.units.as_ref().is_some_and(|(_, unit)| unit == "bpm");
        if our == "activity_type" {
            if !ACTIVITY_TYPE_BASES.contains(&basis.as_str()) {
                return Err(format!("unknown basis {basis}"));
            }
        } else if our == "hrv_rmssd" || our == "hrv_sdnn" || heart_rate_summary {
            if !LINE_BASES.contains(&basis.as_str()) {
                return Err(if VALUE_BASES.contains(&basis.as_str()) {
                    format!("a mapping line cannot claim basis {basis}; use one of {}", LINE_BASES.join(", "))
                } else {
                    format!("unknown basis {basis}")
                });
            }
            if RMSSD_BASES.contains(&basis.as_str()) && our != "hrv_rmssd" {
                return Err(format!("basis {basis} is only used for hrv_rmssd"));
            }
        } else {
            return Err("basis is only used for activity_type, hrv_rmssd, hrv_sdnn, and heart-rate summaries".into());
        }
    }
    match (&kind, &line.scale) {
        (Target::Score(_), None) => return Err(format!("provider_score {} needs a scale", line.their_field)),
        (Target::Score(_), Some(_)) | (_, None) => {}
        (_, Some(_)) => return Err("scale is only used for provider_score".into()),
    }
    let checked = Line {
        their_field: line.their_field.clone(),
        our_field: line.our_field.clone(),
        target: kind.clone(),
        units: line.units.clone(),
        basis: line.basis.clone(),
        scale: line.scale.clone(),
        when_absent: line.when_absent,
        when_writer: line.when_writer.clone(),
    };
    if matches!(kind, Target::WorkoutText) {
        if line.units.is_some() {
            return Err(format!("{our} takes no unit"));
        }
        if our == "activity_type" && line.basis.is_none() {
            return Err(format!("activity_type needs a basis: one of {}", ACTIVITY_TYPE_BASES.join(", ")));
        }
        return Ok(checked);
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
    Ok(checked)
}

/// Each section names the fields that hold its rows. Nothing is assumed (D193).
fn check_settings(section: &Section) -> std::result::Result<Layout, String> {
    let timeseries = section.name == "timeseries";
    let (required, optional): (&[&str], &[&str]) =
        if timeseries { (SAMPLE_SETTINGS, SAMPLE_OPTIONAL_SETTINGS) } else { (SESSION_SETTINGS, &[]) };
    for key in required {
        if section.setting(key).is_none() {
            return Err(format!("[{}] needs {key}", section.name));
        }
    }
    for (key, _) in &section.settings {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(format!("{key} is not a setting of [{}]", section.name));
        }
    }
    let field = |key: &str| section.setting(key).unwrap_or_default().to_string();
    if !timeseries {
        return Ok(Layout::Session {
            id_field: field("id_field"),
            start_field: field("start_field"),
            end_field: field("end_field"),
        });
    }
    let type_field = field("type_field");
    for line in &section.lines {
        if statistic_name(section, &line.their_field) == line.their_field.as_str() {
            return Err(format!("a [timeseries] line reads {type_field}.<type>, not {}", line.their_field));
        }
    }
    Ok(Layout::Samples {
        time_field: field("time_field"),
        type_field,
        value_field: field("value_field"),
        unit_field: field("unit_field"),
        aggregate_field: section.setting("aggregate_field").map(str::to_string),
        window_field: section.setting("window_field").map(str::to_string),
    })
}

fn check_page(file: &MappingFile) -> std::result::Result<Option<Page>, String> {
    let page = &file.page;
    let described = page.rows.is_some()
        || page.marker.is_some()
        || page.section.is_some()
        || page.resolution.is_some()
        || page.raw_resolution.is_some();
    if !described {
        return Ok(None);
    }
    for (key, value) in [("page_rows", &page.rows), ("page_marker", &page.marker), ("page_section", &page.section)] {
        if value.is_none() {
            return Err(format!("the mapping file needs {key} when it describes a page"));
        }
    }
    let section = page.section.clone().unwrap_or_default();
    if file.section(&section).is_none() {
        return Err(format!("page_section {section} is not a section of this file"));
    }
    if page.resolution.is_some() != page.raw_resolution.is_some() {
        return Err("page_resolution and raw_resolution go together".into());
    }
    Ok(Some(Page {
        rows: page.rows.clone().unwrap_or_default(),
        marker: page.marker.clone().unwrap_or_default(),
        section,
        resolution: page.resolution.clone().zip(page.raw_resolution.clone()),
    }))
}

/// Convert by a fixed pair (D180). A whole result is an integer JSON number.
pub fn convert_unit(value: f64, their_unit: &str, our_unit: &str) -> std::result::Result<Value, String> {
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
        return serde_json::json!({
            "timestamp_text": text,
            "utc_offset_text": "Z",
            "utc_offset_meaning": "documented_recording_offset",
            "timestamp_basis": "source_instant"
        });
    }
    let offset = trimmed.get(trimmed.len().saturating_sub(6)..).unwrap_or("");
    serde_json::json!({
        "timestamp_text": text,
        "utc_offset_text": offset,
        "utc_offset_meaning": "meaning_not_documented",
        "timestamp_basis": "source_offset_timestamp"
    })
}

fn utc_text(text: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(text.trim()).ok().map(|time| format_datetime(time.with_timezone(&chrono::Utc)))
}

/// Escape one part of a match key; then the parts are joined with `:` (D181).
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

fn sensor_part(sensor: &str) -> &str {
    if sensor.is_empty() {
        "-"
    } else {
        sensor
    }
}

/// The applied line for each of their fields: a matching `when-writer` line
/// wins, then the line without `when-writer`.
fn applied_lines<'a>(section: &'a MappedSection, writer: &str) -> Vec<&'a Line> {
    let names_writer =
        |line: &Line| line.when_writer.as_ref().is_some_and(|writers| writers.iter().any(|name| name == writer));
    let mut chosen: Vec<usize> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for line in &section.lines {
        if seen.contains(&line.their_field.as_str()) {
            continue;
        }
        seen.push(&line.their_field);
        let same_field = || section.lines.iter().enumerate().filter(|(_, other)| other.their_field == line.their_field);
        let pick = same_field()
            .find(|(_, candidate)| names_writer(candidate))
            .or_else(|| same_field().find(|(_, candidate)| candidate.when_writer.is_none()));
        if let Some((index, _)) = pick {
            chosen.push(index);
        }
    }
    section.lines.iter().enumerate().filter(|(index, _)| chosen.contains(index)).map(|(_, line)| line).collect()
}

fn source_value(converted: Value, our_unit: &str, input: &Value, their_unit: &str) -> Value {
    serde_json::json!({
        "standard_value": converted,
        "standard_unit": our_unit,
        "input_value": input,
        "input_unit": their_unit
    })
}

fn positive_point(quantity: &str, value: &Value) -> bool {
    quantity == "temperature_deviation" || value.as_f64().is_some_and(|number| number > 0.0)
}

fn basis_value(value: &str) -> Value {
    serde_json::json!({ "value": value })
}

// ---- The reader --------------------------------------------------------------

/// A mapped JSON input with its checked mapping.
pub struct MappedReader {
    pub mapping: Mapping,
    pub input: PathBuf,
}

impl MappedReader {
    pub fn new(mapping: Mapping, input: &Path) -> MappedReader {
        MappedReader { mapping, input: input.to_path_buf() }
    }
}

impl RouteReader for MappedReader {
    fn route(&self) -> Route {
        Route {
            provider: self.mapping.name.clone(),
            label: format!("{} import", self.mapping.name),
            vendor_label: self.mapping.name.clone(),
            data_route: self.mapping.route.clone(),
            container: String::new(),
            match_key_method: self.mapping.method(),
            policy: MAPPED_POLICY,
            report: ReportKind::Mapped { mapping: self.mapping.name.clone() },
        }
    }

    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()> {
        let text = read_input(&self.input)?;
        let input: Value =
            serde_json::from_str(&text).map_err(|error| Error::refused(format!("the input is not JSON: {error}")))?;
        let root = input.as_object().ok_or_else(|| Error::refused("the input must be a JSON object"))?;
        refuse_personal_reports(root)?;
        Run { mapping: &self.mapping, sink, keys: HashSet::new(), devices: HashSet::new() }.map_input(root)
    }
}

fn read_input(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_MAPPED_INPUT_BYTES {
        return Err(Error::refused(format!("the input exceeds {MAX_MAPPED_INPUT_BYTES} bytes: {}", path.display())));
    }
    Ok(std::fs::read_to_string(path)?)
}

fn refuse_personal_reports(root: &Map<String, Value>) -> Result<()> {
    let personal =
        |object: &Map<String, Value>| object.contains_key("report_type") || object.contains_key("reported_via");
    if personal(root) {
        return Err(Error::refused(REFUSE_PERSONAL_REPORT));
    }
    for value in root.values() {
        for row in value.as_array().into_iter().flatten() {
            if row.as_object().is_some_and(personal) {
                return Err(Error::refused(REFUSE_PERSONAL_REPORT));
            }
        }
    }
    Ok(())
}

struct Run<'m, 's, 'a> {
    mapping: &'m Mapping,
    sink: &'s mut CandidateSink<'a>,
    keys: HashSet<String>,
    devices: HashSet<(String, String)>,
}

struct Device {
    writer: String,
    sensor: String,
    fields: Map<String, Value>,
}

impl<'m> Run<'m, '_, '_> {
    fn map_input(&mut self, root: &Map<String, Value>) -> Result<()> {
        let resolution = self
            .mapping
            .page
            .as_ref()
            .and_then(|page| page.resolution.as_ref())
            .and_then(|(path, _)| path_text(root, path));
        let mut rows: Vec<(String, &Value)> = Vec::new();
        match self.page_rows(root) {
            Some((section_name, data)) => {
                if let Some(first) = data.first() {
                    if self.row_fits(section_name, first) {
                        rows.extend(data.iter().map(|row| (section_name.to_string(), row)));
                    } else {
                        let rows_field = self.mapping.page.as_ref().map(|page| page.rows.as_str()).unwrap_or_default();
                        self.sink.leave_out(format!(
                            "envelope is not a {section_name} page: pass its {rows_field} array under workout or sleep"
                        ));
                    }
                }
            }
            None => {
                for (key, value) in root {
                    let Some(items) = value.as_array() else {
                        continue;
                    };
                    if items.is_empty() {
                        continue;
                    }
                    if self.mapping.section(key).is_some() {
                        rows.extend(items.iter().map(|row| (key.clone(), row)));
                    } else {
                        self.sink.leave_out(format!("{key}: no mapping section"));
                    }
                }
            }
        }
        for (index, (section_name, row)) in rows.iter().enumerate() {
            let section = self.mapping.section(section_name).expect("rows only name mapped sections");
            let Some(object) = row.as_object() else {
                self.sink.leave_out(format!("{section_name} row {}: not an object", index + 1));
                continue;
            };
            match &section.layout {
                Layout::Samples { .. } => self.map_sample(section, object, resolution.as_deref())?,
                Layout::Session { .. } => self.map_session(section, object)?,
            }
        }
        Ok(())
    }

    /// The rows of a page, and the section they map with, when the input is a page.
    fn page_rows<'r>(&self, root: &'r Map<String, Value>) -> Option<(&'m str, &'r Vec<Value>)> {
        let page = self.mapping.page.as_ref()?;
        let rows = root.get(&page.rows)?.as_array()?;
        root.contains_key(&page.marker).then_some(())?;
        Some((page.section.as_str(), rows))
    }

    /// Whether a row has the fields its section reads. A sample row also has
    /// none of the start fields the file's session sections read.
    fn row_fits(&self, section_name: &str, row: &Value) -> bool {
        let (Some(object), Some(section)) = (row.as_object(), self.mapping.section(section_name)) else {
            return false;
        };
        let fields: Vec<&str> = match &section.layout {
            Layout::Session { id_field, start_field, end_field } => vec![id_field, start_field, end_field],
            Layout::Samples { time_field, type_field, value_field, unit_field, .. } => {
                vec![time_field, type_field, value_field, unit_field]
            }
        };
        if !fields.iter().all(|field| path_value(object, field).is_some()) {
            return false;
        }
        match section.layout {
            Layout::Session { .. } => true,
            Layout::Samples { .. } => self
                .mapping
                .sections
                .iter()
                .filter_map(|other| match &other.layout {
                    Layout::Session { start_field, .. } => Some(start_field),
                    Layout::Samples { .. } => None,
                })
                .all(|start| path_value(object, start).is_none()),
        }
    }

    fn device(&self, row: &Map<String, Value>, writer: &str) -> Device {
        let sensor = self.mapping.sensor.iter().filter_map(|path| path_text(row, path)).collect::<Vec<_>>().join("|");
        let first = |paths: &[String]| paths.iter().find_map(|path| path_text(row, path));
        let mut fields = Map::new();
        for (key, paths) in [
            ("name", &self.mapping.device_name),
            ("model", &self.mapping.device_model),
            ("hardware", &self.mapping.device_hardware),
        ] {
            if let Some(value) = first(paths) {
                fields.insert(key.into(), Value::String(value));
            }
        }
        Device { writer: writer.to_string(), sensor, fields }
    }

    fn used_paths(&self) -> Vec<String> {
        let mut paths = vec![self.mapping.writer_field.clone()];
        for list in
            [&self.mapping.sensor, &self.mapping.device_name, &self.mapping.device_model, &self.mapping.device_hardware]
        {
            paths.extend(list.iter().cloned());
        }
        paths
    }

    /// A row whose `personal_when` field has the named value is a personal report.
    fn personal_report(&self, row: &Map<String, Value>) -> Option<String> {
        let (field, value) = self.mapping.personal_when.as_ref()?;
        (path_text(row, field).as_deref() == Some(value.as_str()))
            .then(|| format!("{field} {value} is a personal report"))
    }

    /// The source for a device, and its `source_device` record the first time
    /// the device is seen. Matching leaves out every record under a deleted source.
    fn source(&mut self, device: &Device) -> Result<String> {
        let source_id = self.sink.route_source(&device.writer, &device.writer, Some(&device.sensor));
        let has_device_fields = !(self.mapping.device_name.is_empty()
            && self.mapping.device_model.is_empty()
            && self.mapping.device_hardware.is_empty());
        if has_device_fields && self.devices.insert((device.writer.clone(), device.sensor.clone())) {
            let mut data = Map::new();
            data.insert("source_id".into(), Value::String(source_id.clone()));
            data.extend(device.fields.clone());
            data.insert("manufacturer".into(), Value::String(device.writer.clone()));
            let key = match_key(&[&device.writer, sensor_part(&device.sensor), "source_device"]);
            self.sink.add(keyed_candidate(RecordType::SourceDevice, data, &key)?)?;
        }
        Ok(source_id)
    }

    fn base(&self, source_id: &str) -> Map<String, Value> {
        let mut data = Map::new();
        data.insert("source_id".into(), Value::String(source_id.into()));
        data.insert("received_at".into(), Value::String(self.sink.received_at().into()));
        data.insert("received_at_basis".into(), Value::String("import_time".into()));
        data
    }

    fn line_basis<'l>(&'l self, line: &'l Line) -> &'l str {
        line.basis.as_deref().unwrap_or(&self.mapping.value_basis)
    }

    fn map_session(&mut self, section: &MappedSection, row: &Map<String, Value>) -> Result<()> {
        let Layout::Session { id_field, start_field, end_field } = &section.layout else {
            return Ok(());
        };
        let name = section.name.as_str();
        let Some(id) = path_text(row, id_field) else {
            self.sink.leave_out(format!("{name}: missing {id_field}"));
            return Ok(());
        };
        let label = format!("{name} {id}");
        let writer_field = &self.mapping.writer_field;
        let Some(writer) = path_text(row, writer_field) else {
            self.sink.leave_out(format!("{label}: missing {writer_field}"));
            return Ok(());
        };
        if let Some(reason) = self.personal_report(row) {
            self.sink.leave_out(format!("{label}: {reason}"));
            return Ok(());
        }
        let (Some(start_text), Some(end_text)) = (path_text(row, start_field), path_text(row, end_field)) else {
            let missing = if path_text(row, start_field).is_none() { start_field } else { end_field };
            self.sink.leave_out(format!("{label}: missing {missing}"));
            return Ok(());
        };
        let Some(start) = utc_text(&start_text) else {
            self.sink.leave_out(format!("{label}: {start_field} is not a timestamp with an offset"));
            return Ok(());
        };
        let Some(end) = utc_text(&end_text) else {
            self.sink.leave_out(format!("{label}: {end_field} is not a timestamp with an offset"));
            return Ok(());
        };
        if end < start {
            self.sink.leave_out(format!("{label}: end is before the start"));
            return Ok(());
        }
        let device = self.device(row, &writer);
        let session_key = match_key(&[&writer, sensor_part(&device.sensor), name, &id]);
        if !self.keys.insert(session_key.clone()) {
            self.sink.leave_out(format!("{label}: duplicate match key {session_key}"));
            return Ok(());
        }
        self.sink.begin_group(label.clone());
        let source_id = self.source(&device)?;
        let mut supplied = row.clone();
        for path in self.used_paths().iter().chain([id_field, start_field, end_field]) {
            remove_path(&mut supplied, path);
        }
        let mut data = self.base(&source_id);
        data.insert("provider_record_id".into(), Value::String(id.clone()));
        data.insert("start_at".into(), Value::String(start.clone()));
        data.insert("end_at".into(), Value::String(end.clone()));
        data.insert("input_start_timestamp".into(), input_receipt(&start_text));
        data.insert("input_end_timestamp".into(), input_receipt(&end_text));
        let mut summaries = Vec::new();
        let mut stages = Map::new();
        let mut children = Vec::new();
        let mut field_basis = Map::new();
        field_basis.insert("value".into(), Value::String(self.mapping.value_basis.clone()));
        let mut written: HashSet<String> = HashSet::new();
        let mut understood = Vec::new();
        // The first line that writes a field wins, and `when-absent` lines come
        // last (D196). A second line for the same field leaves its value under
        // the name it arrived with.
        let lines = applied_lines(section, &writer);
        let ordered = lines.iter().filter(|line| !line.when_absent).chain(lines.iter().filter(|line| line.when_absent));
        for line in ordered {
            if written.contains(&line.our_field) {
                continue;
            }
            let Some(value) = path_value(row, &line.their_field).cloned() else {
                continue;
            };
            if value.is_null() {
                remove_path(&mut supplied, &line.their_field);
                continue;
            }
            if let Target::WorkoutText = line.target {
                let Some(text) = value.as_str().filter(|text| !text.trim().is_empty()) else {
                    continue;
                };
                data.insert(line.our_field.clone(), Value::String(text.to_string()));
                if let Some(basis) = line.basis.as_ref().filter(|_| line.our_field == "activity_type") {
                    field_basis.insert("activity_type".into(), Value::String(basis.clone()));
                }
            } else {
                let Some(number) = value.as_f64() else {
                    continue;
                };
                let (their_unit, our_unit) = line.units.clone().unwrap_or_default();
                let converted = convert_unit(number, &their_unit, &our_unit).map_err(Error::refused)?;
                match &line.target {
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
                        summary["summary_type"] = Value::String(summary_type.clone());
                        summaries.push(summary);
                    }
                    Target::Point(_) => {
                        if !positive_point(&line.our_field, &converted) {
                            continue;
                        }
                        let mut child = self.base(&source_id);
                        child.insert("quantity".into(), Value::String(line.our_field.clone()));
                        child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                        child.insert("field_basis".into(), basis_value(self.line_basis(line)));
                        children.push((RecordType::PointMeasurement, "point", line.our_field.clone(), child));
                    }
                    Target::Summary(summary_type) => {
                        let mut child = self.base(&source_id);
                        child.insert("summary_type".into(), Value::String(summary_type.clone()));
                        child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                        child.insert("field_basis".into(), basis_value(self.line_basis(line)));
                        children.push((RecordType::ProviderSummary, "summary", summary_type.clone(), child));
                    }
                    Target::Score(metric) => {
                        let mut child = self.base(&source_id);
                        child.insert("metric".into(), Value::String(metric.clone()));
                        child.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
                        child.insert("scale".into(), Value::String(line.scale.clone().unwrap_or_default()));
                        child.insert("field_basis".into(), basis_value("provider"));
                        children.push((RecordType::ProviderScore, "score", metric.clone(), child));
                    }
                    Target::WorkoutText => {}
                }
            }
            written.insert(line.our_field.clone());
            remove_path(&mut supplied, &line.their_field);
            understood.push(line.their_field.clone());
        }
        for field in understood {
            self.sink.understood_under(&source_id, field);
        }
        if !stages.is_empty() {
            data.insert("stage_minutes".into(), Value::Object(stages));
        }
        data.insert("source_reported_summaries".into(), Value::Array(summaries));
        data.insert("field_basis".into(), Value::Object(field_basis));
        if !supplied.is_empty() {
            for key in supplied.keys() {
                self.sink.preserved_under(&source_id, key.clone());
            }
            data.insert("supplied_fields".into(), Value::Object(supplied));
        }
        let record_type = if name == "sleep" { RecordType::SleepSession } else { RecordType::WorkoutSession };
        self.sink.add(keyed_candidate(record_type, data, &session_key)?)?;
        for (record_type, kind, name_part, mut child) in children {
            child.insert("provider_record_id".into(), Value::String(id.clone()));
            child.insert("recorded_at".into(), Value::String(end.clone()));
            child.insert("input_timestamp".into(), input_receipt(&end_text));
            let key = format!("{session_key}:{}", match_key(&[kind, &name_part]));
            self.keys.insert(key.clone());
            self.sink.add(keyed_candidate(record_type, child, &key)?)?;
        }
        Ok(())
    }

    fn map_sample(
        &mut self,
        section: &MappedSection,
        row: &Map<String, Value>,
        resolution: Option<&str>,
    ) -> Result<()> {
        let Layout::Samples { time_field, type_field, value_field, unit_field, aggregate_field, window_field } =
            &section.layout
        else {
            return Ok(());
        };
        let sample_type = path_text(row, type_field).unwrap_or_default();
        let time_text = path_text(row, time_field);
        let utc = time_text.as_deref().and_then(utc_text);
        let label = format!(
            "timeseries {} {}",
            if sample_type.is_empty() { "(no type)" } else { &sample_type },
            utc.clone().or_else(|| time_text.clone()).unwrap_or_else(|| "(no timestamp)".into())
        );
        let present = |key: &str| path_value(row, key).is_some_and(|value| !value.is_null());
        if aggregate_field.as_deref().is_some_and(present) {
            self.sink.leave_out(format!("{label}: aggregate row, not a raw sample"));
            return Ok(());
        }
        let raw = self.mapping.page.as_ref().and_then(|page| page.resolution.as_ref()).map(|(_, raw)| raw.as_str());
        if let Some(resolution) = resolution.filter(|value| Some(*value) != raw) {
            self.sink.leave_out(format!("{label}: resolution {resolution}: not raw samples"));
            return Ok(());
        }
        if let Some(window) = window_field.as_deref().filter(|field| resolution.is_none() && present(field)) {
            self.sink.leave_out(format!("{label}: {window} present without resolution: cannot confirm raw"));
            return Ok(());
        }
        let (Some(time_text), Some(utc)) = (time_text, utc) else {
            self.sink.leave_out(format!("{label}: missing {time_field}"));
            return Ok(());
        };
        if sample_type.is_empty() {
            self.sink.leave_out(format!("{label}: missing {type_field}"));
            return Ok(());
        }
        let writer_field = &self.mapping.writer_field;
        let Some(writer) = path_text(row, writer_field) else {
            self.sink.leave_out(format!("{label}: missing {writer_field}"));
            return Ok(());
        };
        if let Some(reason) = self.personal_report(row) {
            self.sink.leave_out(format!("{label}: {reason}"));
            return Ok(());
        }
        let their_field = format!("{type_field}.{sample_type}");
        let Some(line) = applied_lines(section, &writer).into_iter().find(|line| line.their_field == their_field)
        else {
            self.sink.leave_out(format!("{label}: unmapped type {sample_type}"));
            return Ok(());
        };
        let (their_unit, our_unit) = line.units.clone().unwrap_or_default();
        let sample_unit = path_text(row, unit_field).unwrap_or_default();
        if sample_unit != their_unit {
            let shown = if sample_unit.is_empty() { "(none)" } else { &sample_unit };
            self.sink.leave_out(format!("{label}: unit {shown} does not match {their_unit}"));
            return Ok(());
        }
        let value = path_value(row, value_field).cloned().unwrap_or(Value::Null);
        let Some(number) = value.as_f64() else {
            self.sink.leave_out(format!("{label}: value is not a number"));
            return Ok(());
        };
        let converted = convert_unit(number, &their_unit, &our_unit).map_err(Error::refused)?;
        if let Target::Point(_) = line.target {
            if !positive_point(&line.our_field, &converted) {
                self.sink.leave_out(format!("{label}: {} must be positive", line.our_field));
                return Ok(());
            }
        }
        let device = self.device(row, &writer);
        let (record_type, kind_word, name_part) = match &line.target {
            Target::Point(_) => (RecordType::PointMeasurement, "point", sample_type.clone()),
            Target::Summary(_) => (RecordType::ProviderSummary, "summary", sample_type.clone()),
            Target::Score(metric) => (RecordType::ProviderScore, "score", metric.clone()),
            _ => return Err(Error::other("a timeseries line writes a point, a summary, or a score")),
        };
        let key = match_key(&[&writer, sensor_part(&device.sensor), "timeseries", kind_word, &name_part, &utc]);
        if !self.keys.insert(key.clone()) {
            self.sink.leave_out(format!("{label}: duplicate match key {key}"));
            return Ok(());
        }
        self.sink.begin_group(label.clone());
        let source_id = self.source(&device)?;
        let mut supplied = row.clone();
        for path in self.used_paths().iter().chain([time_field, value_field, type_field, unit_field]) {
            remove_path(&mut supplied, path);
        }
        let mut data = self.base(&source_id);
        match &line.target {
            Target::Point(_) => {
                data.insert("quantity".into(), Value::String(line.our_field.clone()));
            }
            Target::Summary(summary_type) => {
                data.insert("summary_type".into(), Value::String(summary_type.clone()));
            }
            Target::Score(metric) => {
                data.insert("metric".into(), Value::String(metric.clone()));
                data.insert("scale".into(), Value::String(line.scale.clone().unwrap_or_default()));
            }
            _ => {}
        }
        data.insert("value".into(), source_value(converted, &our_unit, &value, &their_unit));
        data.insert("recorded_at".into(), Value::String(utc));
        data.insert("input_timestamp".into(), input_receipt(&time_text));
        let basis = if matches!(line.target, Target::Score(_)) { "provider" } else { self.line_basis(line) };
        data.insert("field_basis".into(), basis_value(basis));
        if !supplied.is_empty() {
            for key in supplied.keys() {
                self.sink.preserved_under(&source_id, key.clone());
            }
            data.insert("supplied_fields".into(), Value::Object(supplied));
        }
        self.sink.understood_under(&source_id, line.their_field.clone());
        self.sink.add(keyed_candidate(record_type, data, &key)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = "# Small\n\n```ath-mapping\nname: small\nroute: small_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\nsleep_duration_seconds: asleep_seconds s s\n```\n";

    const SAMPLES: &str = "```ath-mapping\nname: samples\nroute: samples_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[timeseries]\ntime_field: at\ntype_field: kind\nvalue_field: reading\nunit_field: unit\nkind.rmssd_night: hrv_rmssd ms ms\n```";

    fn refusal(text: &str) -> String {
        load_mapping(text).unwrap_err().to_string()
    }

    #[test]
    fn a_crlf_mapping_file_parses_the_same() {
        let lf = parse_mapping(SMALL).unwrap();
        let crlf = parse_mapping(&SMALL.replace('\n', "\r\n")).unwrap();
        assert_eq!(lf, crlf);
        assert_eq!(check_mapping(&crlf).unwrap(), check_mapping(&lf).unwrap());
    }

    #[test]
    fn one_hundred_seconds_is_the_binary64_minute() {
        let value = convert_unit(100.0, "s", "min").unwrap();
        assert_eq!(value.as_f64(), Some(1.6666666666666667));
        assert_eq!(crate::json::to_string(&value), "1.6666666666666667");
    }

    #[test]
    fn whole_seconds_are_an_integer_value() {
        assert_eq!(format!("{:?}", convert_unit(600.0, "s", "s").unwrap()), "Number(600)");
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
        assert_eq!(
            parse_mapping("# nothing here").unwrap_err().to_string(),
            "the mapping file has no ath-mapping block"
        );
        assert_eq!(
            parse_mapping("```ath-mapping\nname: x\n").unwrap_err().to_string(),
            "the ath-mapping block is not closed"
        );
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
        assert_eq!(
            refusal(&SMALL.replace("id_field: id\n", "id_field: id\ntime_field: at\n")),
            "time_field is not a setting of [sleep]"
        );
        load_mapping(SAMPLES).unwrap();
    }

    #[test]
    fn a_timeseries_line_reads_the_type_field() {
        let text = SAMPLES.replace("kind.rmssd_night:", "rmssd_night:");
        assert_eq!(refusal(&text), "a [timeseries] line reads kind.<type>, not rmssd_night");
    }

    #[test]
    fn a_page_is_described_in_full() {
        let page = SAMPLES
            .replace("value_basis: provider\n", "value_basis: provider\npage_rows: rows\npage_section: timeseries\n");
        assert_eq!(refusal(&page), "the mapping file needs page_marker when it describes a page");
        let elsewhere = page.replace("page_section: timeseries\n", "page_marker: meta\npage_section: sleep\n");
        assert_eq!(refusal(&elsewhere), "page_section sleep is not a section of this file");
        let half = page.replace(
            "page_section: timeseries\n",
            "page_marker: meta\npage_section: timeseries\npage_resolution: meta.resolution\n",
        );
        assert_eq!(refusal(&half), "page_resolution and raw_resolution go together");
        let full = half
            .replace("page_resolution: meta.resolution\n", "page_resolution: meta.resolution\nraw_resolution: raw\n");
        let mapping = load_mapping(&full).unwrap();
        assert_eq!(
            mapping.page,
            Some(Page {
                rows: "rows".into(),
                marker: "meta".into(),
                section: "timeseries".into(),
                resolution: Some(("meta.resolution".into(), "raw".into())),
            })
        );
    }

    #[test]
    fn personal_when_names_a_field_and_a_value() {
        let text = SMALL.replace("value_basis: provider\n", "value_basis: provider\npersonal_when: origin typed\n");
        assert_eq!(parse_mapping(&text).unwrap().personal_when, Some(("origin".to_string(), "typed".to_string())));
        let broken = SMALL.replace("value_basis: provider\n", "value_basis: provider\npersonal_when: origin\n");
        assert_eq!(parse_mapping(&broken).unwrap_err().to_string(), "could not read 'personal_when: origin'");
    }

    #[test]
    fn an_hrv_value_names_its_statistic_in_any_record() {
        assert_eq!(
            refusal(&SMALL.replace(
                "sleep_duration_seconds: asleep_seconds s s",
                "nightly_hrv: provider_summary.hrv_rmssd ms ms"
            )),
            "refusing to map nightly_hrv onto provider_summary.hrv_rmssd; the field does not name the statistic"
        );
        assert_eq!(
            refusal(&SMALL.replace(
                "sleep_duration_seconds: asleep_seconds s s",
                "hrv_rmssd_night: provider_score.sdnn_balance score score scale 0-100"
            )),
            "refusing to map hrv_rmssd_night onto provider_score.sdnn_balance; the field does not name the statistic"
        );
        assert_eq!(
            refusal(&SAMPLES.replace("kind.rmssd_night:", "kind.sdnn_night:")),
            "refusing to map sdnn_night onto hrv_rmssd; the field does not name the statistic"
        );
        let summary = SMALL
            .replace("sleep_duration_seconds: asleep_seconds s s", "rmssd_night: provider_summary.hrv_rmssd ms ms");
        load_mapping(&summary).unwrap();
    }

    #[test]
    fn a_field_without_the_statistic_needs_an_rmssd_basis_and_writers() {
        let line = |rest: &str| {
            SMALL.replace("sleep_duration_seconds: asleep_seconds s s", &format!("nightly: hrv_rmssd ms ms{rest}"))
        };
        let refused = "refusing to map nightly onto hrv_rmssd; the field does not name the statistic";
        assert_eq!(refusal(&line("")), refused);
        assert_eq!(refusal(&line(" basis hrv_mean_rmssd")), refused);
        assert_eq!(refusal(&line(" basis provider when-writer a")), refused);
        load_mapping(&line(" basis hrv_mean_rmssd when-writer a,b")).unwrap();
    }

    #[test]
    fn a_line_claims_only_a_basis_a_mapping_can_know() {
        let line = |basis: &str| {
            SMALL.replace(
                "sleep_duration_seconds: asleep_seconds s s",
                &format!("rmssd_night: hrv_rmssd ms ms basis {basis}"),
            )
        };
        for basis in ["entered_by_hand", "calculated_on_import"] {
            assert_eq!(
                refusal(&line(basis)),
                format!(
                    "a mapping line cannot claim basis {basis}; use one of provider, provider_or_aggregator, hrv_mean_rmssd, provider_documented_rmssd"
                )
            );
        }
        let heart_rate = SMALL.replace(
            "sleep_duration_seconds: asleep_seconds s s",
            "avg_hr: provider_summary.average_heart_rate bpm bpm basis hrv_mean_rmssd",
        );
        assert_eq!(refusal(&heart_rate), "basis hrv_mean_rmssd is only used for hrv_rmssd");
        load_mapping(&line("provider_or_aggregator")).unwrap();
    }

    #[test]
    fn the_bases_are_the_record_format_bases() {
        let values: Vec<&str> = crate::model::ValueBasis::ALL.iter().map(|basis| basis.as_str()).collect();
        assert_eq!(values, VALUE_BASES);
        let activity: Vec<&str> = crate::model::ActivityTypeBasis::ALL.iter().map(|basis| basis.as_str()).collect();
        assert_eq!(activity, ACTIVITY_TYPE_BASES);
    }

    #[test]
    fn a_line_target_is_typed() {
        let mapping = load_mapping(SMALL).unwrap();
        assert_eq!(mapping.sections[0].lines[0].target, Target::SleepField("s"));
        assert_eq!(
            mapping.sections[0].layout,
            Layout::Session { id_field: "id".into(), start_field: "start_time".into(), end_field: "end_time".into() }
        );
    }
}
