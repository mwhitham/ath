//! Reading `export.xml` as a stream of events.
//!
//! The 2.1.0 helper read the whole file into a list of events and handed the
//! list to JavaScript. This reader keeps the helper's rules and sends each
//! event to a handler the moment the helper would have listed it, so nothing
//! grows with the size of the export here.
//!
//! - A `Record` is a point measurement, a series sample, a sleep fragment, or
//!   an unmapped type that is counted. Values are converted with the unit
//!   rules in `units.rs`.
//! - A sleep fragment is announced when its `Record` opens, so its writer is
//!   resolved in the order 2.1.0 resolved it. Points and samples are sent when
//!   their `Record` closes, after any `HKTimeZone` metadata inside it.
//! - SDNN samples keep the beats listed under them, placed in their window by
//!   minute and second. The hour is not read, because Apple writes beat times
//!   on a 12- or 24-hour clock in the phone's language.
//! - A `Workout` is sent when it closes, with its laps, average and maximum
//!   heart rate, inline route points, and `FileReference` paths.
//! - An `ActivitySummary` becomes provider summaries for its calendar day.
//!
//! Dates are read with the helper's own rules: `parse_apple_date` keeps the
//! text and its offset, and `epoch_ms` is chrono's RFC 3339 reading.

use std::collections::HashMap;
use std::io::BufRead;

use chrono::{DateTime, Utc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::error::{Error, Result};
use crate::json::{Map, Value};
use crate::model::*;

use super::super::units;
use super::hrv::{helper_round, rmssd_from_beats, Beat, Rmssd};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const BEAT_WINDOW_TOLERANCE_MS: i64 = 5 * 60 * 1000;
const LONGEST_PLACEABLE_WINDOW_MS: i64 = 55 * 60 * 1000;

/// What the reader found, in the order 2.1.0 found it.
pub trait Events {
    /// A sleep `Record` opened. Its writer is resolved now.
    fn sleep_opened(&mut self, writer: &str, device_raw: &str, on_day: &str) -> Result<()>;
    /// A sleep `Record` closed, with any timezone found inside it.
    fn sleep_fragment(&mut self, fragment: SleepFragment) -> Result<()>;
    fn point(&mut self, point: PointRecord) -> Result<()>;
    fn sample(&mut self, sample: SeriesRecord) -> Result<()>;
    fn workout(&mut self, workout: WorkoutRecord) -> Result<()>;
    fn activity(&mut self, activity: ActivityRecord) -> Result<()>;
    /// An omitted object, with the first example kept.
    fn skip(&mut self, reason: &str, example: Option<String>, count: u64);
}

/// A sleep stage interval.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SleepFragment {
    pub writer: String,
    pub device_raw: String,
    pub start: String,
    pub end: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub input_start: String,
    pub input_end: String,
    pub value: String,
    pub source_time_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointRecord {
    pub writer: String,
    pub device_raw: String,
    pub on_day: String,
    pub quantity: PointMeasurementType,
    pub value: f64,
    /// The parsed date text, with its offset.
    pub recorded_at: String,
    pub input_raw: String,
    pub input_value: f64,
    pub input_unit: Option<String>,
    pub source_time_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeriesRecord {
    pub writer: String,
    pub device_raw: String,
    pub on_day: String,
    pub quantity: SeriesQuantity,
    /// The parsed start text, with its offset.
    pub at: String,
    pub value: f64,
    pub duration_ms: i64,
    pub input_raw: String,
    pub input_value: f64,
    pub input_unit: Option<String>,
    pub source_time_zone: Option<String>,
    pub ends_at: Option<String>,
    /// Beats under an SDNN sample; `None` for other quantities.
    pub beats: Option<Vec<Beat>>,
}

/// A beat as a sample: its instant written with the record's offset, and its
/// interval rounded to a tenth of a millisecond.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatSample {
    pub at: String,
    pub value: f64,
}

impl SeriesRecord {
    /// The beat samples and the RMSSD the helper derived from them.
    pub fn beat_samples(&self) -> (Vec<BeatSample>, Option<Rmssd>) {
        let Some(beats) = self.beats.as_ref().filter(|beats| !beats.is_empty()) else {
            return (Vec::new(), None);
        };
        let start_ms = epoch_ms(&self.at).unwrap_or(0);
        let samples = beats
            .iter()
            .map(|beat| BeatSample {
                at: at_offset_of(&self.at, start_ms as f64 + beat.offset_ms as f64),
                value: helper_round(beat.interval_ms * 10.0) / 10.0,
            })
            .collect();
        (samples, rmssd_from_beats(beats))
    }
}

/// A lap.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lap {
    pub label: String,
    pub duration_s: Option<f64>,
}

/// A value the workout's source wrote, with its unit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WrittenValue {
    pub input_value: f64,
    pub input_unit: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkoutRecord {
    pub writer: String,
    pub device_raw: String,
    pub on_day: String,
    pub start: String,
    pub end: String,
    pub input_start: String,
    pub input_end: String,
    /// `HKWorkoutActivityType` without its prefix, lowercase. Empty when absent.
    pub activity: String,
    /// Session totals by their 2.1.0 names, in the order found.
    /// A total JSON could not hold, such as an overflow to infinity, is `None`:
    /// 2.1.0 wrote it as `null` and left it out of the summaries.
    pub aggregates: Vec<(String, Option<f64>)>,
    pub input_summaries: Vec<(String, WrittenValue)>,
    pub laps: Vec<Lap>,
    pub route: Vec<RoutePoint>,
    /// One object per `WorkoutRoute`, stored as `supplied_fields.WorkoutRoute`.
    pub route_fields: Vec<Value>,
    pub file_references: Vec<String>,
    pub source_time_zone: Option<String>,
}

/// One provider summary from an `ActivitySummary`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityValue {
    pub summary_type: &'static str,
    pub value: f64,
    pub standard_unit: &'static str,
    pub input_value: f64,
    pub input_unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRecord {
    pub writer: String,
    pub device_raw: String,
    pub on_day: String,
    pub date: String,
    pub values: Vec<ActivityValue>,
}

/// Stream `export.xml` to a handler. An XML error ends the import with the
/// 2.1.0 helper's message.
pub fn read<R: BufRead>(source: R, events: &mut dyn Events) -> Result<()> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(true);
    let mut state = State::default();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                state.on_open(&name, attributes(&event), events)?;
            }
            Ok(Event::Empty(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                state.on_open(&name, attributes(&event), events)?;
                state.on_close(&name, events)?;
            }
            Ok(Event::End(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                state.on_close(&name, events)?;
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(Error::other(format!("Apple Health XML: {error}"))),
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

type Attributes = HashMap<String, String>;

/// Attribute values unescaped; a malformed attribute is left out, and the
/// first of two with one name is kept, as the helper kept it.
fn attributes(event: &BytesStart<'_>) -> Attributes {
    let mut map = HashMap::new();
    for attribute in event.attributes().flatten() {
        let key = String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned();
        let value = attribute.unescape_value().map(|text| text.into_owned()).unwrap_or_default();
        map.insert(key, value);
    }
    map
}

enum Pending {
    Point(PointRecord),
    Series(SeriesRecord),
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

struct RouteNote {
    fields: Map<String, Value>,
    metadata: Vec<Value>,
    file_reference: Option<String>,
    location_count: usize,
}

struct OpenWorkout {
    record: WorkoutRecord,
    notes: Vec<RouteNote>,
    in_route: bool,
}

#[derive(Default)]
struct State {
    pending: Option<Pending>,
    workout: Option<OpenWorkout>,
    /// A `WorkoutRoute` outside a workout. Its locations are not a route.
    orphan_route: bool,
}

impl State {
    fn on_open(&mut self, name: &str, attrs: Attributes, events: &mut dyn Events) -> Result<()> {
        match name {
            "Record" => {
                self.pending = None;
                self.open_record(attrs, events)?;
            }
            "ActivitySummary" => self.add_activity(&attrs, events)?,
            "Workout" => self.open_workout(&attrs, events),
            "WorkoutRoute" => self.open_route(&attrs, events),
            "FileReference" => self.add_file_reference(&attrs),
            "Location" => self.add_location(&attrs, events),
            "WorkoutEvent" => self.add_lap(&attrs),
            "MetadataEntry" => self.add_metadata(&attrs),
            "InstantaneousBeatsPerMinute" => self.add_beat(&attrs, events),
            "ClinicalRecord" => events.skip("clinical records (out of scope)", None, 1),
            _ => {}
        }
        Ok(())
    }

    fn on_close(&mut self, name: &str, events: &mut dyn Events) -> Result<()> {
        if name == "Record" {
            match self.pending.take() {
                Some(Pending::Sleep(fragment)) => events.sleep_fragment(fragment)?,
                Some(Pending::Point(point)) => events.point(point)?,
                Some(Pending::Series(series)) => events.sample(series)?,
                None => {}
            }
        }
        if name == "WorkoutRoute" {
            self.close_route(events);
        }
        if name == "Workout" {
            if let Some(workout) = self.workout.take() {
                events.workout(workout.finish())?;
            }
        }
        Ok(())
    }

    fn open_record(&mut self, attrs: Attributes, events: &mut dyn Events) -> Result<()> {
        let raw_type = attrs.get("type").cloned().unwrap_or_default();
        let identifier = identifier_of(&raw_type);
        let start_raw = attrs.get("startDate").cloned().unwrap_or_default();
        let end_raw = attrs.get("endDate").cloned().unwrap_or_default();
        let start = parse_apple_date(&start_raw);
        let end = parse_apple_date(&end_raw);
        let unit = attrs.get("unit").map(String::as_str).unwrap_or("");
        let shown_unit = attrs.get("unit").map(String::as_str).unwrap_or("(none)");

        if identifier == "SleepAnalysis" {
            let (Some(start), Some(end)) = (start, end) else {
                events.skip("sleep records with an unreadable date", None, 1);
                return Ok(());
            };
            let fragment = SleepFragment {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                start_ms: epoch_ms(&start).unwrap_or(0),
                end_ms: epoch_ms(&end).unwrap_or(0),
                start,
                end,
                input_start: start_raw.clone(),
                input_end: end_raw,
                value: attrs.get("value").cloned().unwrap_or_default(),
                source_time_zone: None,
            };
            events.sleep_opened(&fragment.writer, &fragment.device_raw, &on_day(&start_raw))?;
            self.pending = Some(Pending::Sleep(fragment));
            return Ok(());
        }

        if let Some((quantity, convert)) = point_mapping(&identifier) {
            let Some(start) = start else {
                events.skip("measurements with an unreadable date", None, 1);
                return Ok(());
            };
            let Some(raw) = attrs.get("value").and_then(|value| parse_f64(value)) else {
                events.skip("measurements with a non-numeric value", None, 1);
                return Ok(());
            };
            let Some(converted) = convert(raw, unit) else {
                events.skip(
                    "measurements in a unit we do not recognize",
                    Some(format!("{identifier} in \"{shown_unit}\"")),
                    1,
                );
                return Ok(());
            };
            if !converted.is_finite() || converted <= 0.0 {
                events.skip("measurements outside a plausible range", None, 1);
                return Ok(());
            }
            self.pending = Some(Pending::Point(PointRecord {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                on_day: on_day(&start_raw),
                quantity,
                value: round2(converted),
                recorded_at: start,
                input_raw: start_raw,
                input_value: raw,
                input_unit: attrs.get("unit").cloned(),
                source_time_zone: None,
            }));
            return Ok(());
        }

        if let Some((quantity, convert)) = series_mapping(&identifier) {
            let Some(start) = start else {
                events.skip("series samples with an unreadable date", None, 1);
                return Ok(());
            };
            let Some(raw) = attrs.get("value").and_then(|value| parse_f64(value)) else {
                events.skip("series samples with a non-numeric value", None, 1);
                return Ok(());
            };
            let Some(converted) = convert(raw, unit) else {
                events.skip(
                    "series samples in a unit we do not recognize",
                    Some(format!("{identifier} in \"{shown_unit}\"")),
                    1,
                );
                return Ok(());
            };
            let duration_ms = match (epoch_ms(&start), end.as_deref().and_then(epoch_ms)) {
                (Some(start_ms), Some(end_ms)) if end_ms > start_ms => end_ms - start_ms,
                _ => 0,
            };
            self.pending = Some(Pending::Series(SeriesRecord {
                writer: writer_name(&attrs),
                device_raw: device_raw(&attrs),
                on_day: on_day(&start_raw),
                quantity,
                at: start,
                value: round3(converted),
                duration_ms,
                input_raw: start_raw,
                input_value: raw,
                input_unit: attrs.get("unit").cloned(),
                source_time_zone: None,
                ends_at: end,
                beats: (quantity == SeriesQuantity::HrvSdnn).then(Vec::new),
            }));
            return Ok(());
        }

        if !raw_type.is_empty() {
            let label = if identifier.is_empty() { raw_type } else { identifier };
            events.skip(&format!("unmapped HealthKit type: {label}"), None, 1);
        }
        Ok(())
    }

    fn add_activity(&mut self, attrs: &Attributes, events: &mut dyn Events) -> Result<()> {
        let date = attrs.get("dateComponents").map(|value| value.trim()).unwrap_or("");
        if date.len() != 10 || !is_calendar_day(date) {
            events.skip(
                "activity summaries with an unusable date",
                Some(attrs.get("dateComponents").cloned().unwrap_or_else(|| "(none)".into())),
                1,
            );
            return Ok(());
        }
        let mut values = Vec::new();
        let energy_unit = attrs.get("activeEnergyBurnedUnit").cloned();
        if attrs.contains_key("activeEnergyBurned") || attrs.contains_key("activeEnergyBurnedGoal") {
            match energy_unit {
                None => {
                    let count = u64::from(attrs.contains_key("activeEnergyBurned"))
                        + u64::from(attrs.contains_key("activeEnergyBurnedGoal"));
                    events.skip("activity energy summaries without a source unit", None, count);
                }
                Some(unit) => {
                    let energy = Some(unit.as_str());
                    push_summary(
                        attrs,
                        &mut values,
                        ("activeEnergyBurned", "active_energy_burned", "kcal"),
                        energy,
                        events,
                    );
                    push_summary(
                        attrs,
                        &mut values,
                        ("activeEnergyBurnedGoal", "active_energy_goal", "kcal"),
                        energy,
                        events,
                    );
                }
            }
        }
        for (attribute, summary_type, unit) in [
            ("appleExerciseTime", "exercise_time", "min"),
            ("appleExerciseTimeGoal", "exercise_time_goal", "min"),
            ("appleStandHours", "stand_hours", "h"),
            ("appleStandHoursGoal", "stand_hours_goal", "h"),
        ] {
            push_summary(attrs, &mut values, (attribute, summary_type, unit), None, events);
        }
        let name = attrs.get("sourceName").map(|value| value.trim()).unwrap_or("");
        events.activity(ActivityRecord {
            writer: if name.is_empty() { "Apple Health".into() } else { name.to_string() },
            device_raw: device_raw(attrs),
            on_day: on_day(attrs.get("startDate").map(String::as_str).unwrap_or("")),
            date: date.to_string(),
            values,
        })
    }

    fn open_workout(&mut self, attrs: &Attributes, events: &mut dyn Events) {
        let start_raw = attrs.get("startDate").cloned().unwrap_or_default();
        let end_raw = attrs.get("endDate").cloned().unwrap_or_default();
        let start = parse_apple_date(&start_raw);
        let end = parse_apple_date(&end_raw);
        let usable = match (start.as_deref().and_then(epoch_ms), end.as_deref().and_then(epoch_ms)) {
            (Some(start_ms), Some(end_ms)) => end_ms > start_ms,
            _ => false,
        };
        let activity_attribute =
            attrs.get("workoutActivityType").map(|value| strip_all(value, "HKWorkoutActivityType"));
        let (Some(start), Some(end), true) = (start, end, usable) else {
            let activity = activity_attribute.filter(|value| !value.is_empty()).unwrap_or("unknown");
            let shown = |raw: &str| if raw.is_empty() { "(none)".to_string() } else { raw.to_string() };
            events.skip(
                "workouts with an unusable time range",
                Some(format!("{activity} from \"{}\" to \"{}\"", shown(&start_raw), shown(&end_raw))),
                1,
            );
            return;
        };
        let mut aggregates = Vec::new();
        let mut input_summaries = Vec::new();
        if let Some(distance) =
            attrs.get("totalDistance").and_then(|value| parse_f64(value)).filter(|value| *value > 0.0)
        {
            let unit = attrs.get("totalDistanceUnit").cloned().unwrap_or_else(|| "m".into());
            let metres = units::metres(distance, &unit).unwrap_or(distance);
            set(&mut aggregates, "distance_m", finite(round2(metres)));
            set(&mut input_summaries, "distance_m", WrittenValue { input_value: distance, input_unit: unit });
        }
        if let Some(energy) =
            attrs.get("totalEnergyBurned").and_then(|value| parse_f64(value)).filter(|value| *value > 0.0)
        {
            let unit = attrs.get("totalEnergyBurnedUnit").cloned().unwrap_or_else(|| "kcal".into());
            set(&mut aggregates, "energy_kcal", finite(round2(energy)));
            set(&mut input_summaries, "energy_kcal", WrittenValue { input_value: energy, input_unit: unit });
        }
        self.workout = Some(OpenWorkout {
            record: WorkoutRecord {
                writer: writer_name(attrs),
                device_raw: device_raw(attrs),
                on_day: on_day(&start_raw),
                start,
                end,
                input_start: start_raw,
                input_end: end_raw,
                activity: activity_attribute.map(str::to_lowercase).unwrap_or_default(),
                aggregates,
                input_summaries,
                laps: Vec::new(),
                route: Vec::new(),
                route_fields: Vec::new(),
                file_references: Vec::new(),
                source_time_zone: None,
            },
            notes: Vec::new(),
            in_route: false,
        });
    }

    fn open_route(&mut self, attrs: &Attributes, events: &mut dyn Events) {
        let Some(workout) = self.workout.as_mut() else {
            self.orphan_route = true;
            events.skip("inline WorkoutRoute outside a workout", Some("WorkoutRoute".into()), 1);
            return;
        };
        workout.in_route = true;
        workout.notes.push(RouteNote {
            fields: sorted_attributes(attrs),
            metadata: Vec::new(),
            file_reference: None,
            location_count: 0,
        });
    }

    fn close_route(&mut self, events: &mut dyn Events) {
        if self.orphan_route {
            self.orphan_route = false;
            return;
        }
        let Some(workout) = self.workout.as_mut().filter(|workout| workout.in_route) else {
            return;
        };
        let empty = workout.notes.last().is_some_and(|note| note.location_count == 0 && note.file_reference.is_none());
        workout.in_route = false;
        if empty {
            let example = format!("workout from \"{}\"", workout.record.input_start);
            events.skip("inline WorkoutRoute had no location points", Some(example), 1);
        }
    }

    fn add_file_reference(&mut self, attrs: &Attributes) {
        let Some(path) = attrs.get("path").map(|value| value.trim()).filter(|value| !value.is_empty()) else {
            return;
        };
        let Some(workout) = self.workout.as_mut().filter(|workout| workout.in_route) else {
            return;
        };
        if let Some(note) = workout.notes.last_mut() {
            note.file_reference = Some(path.to_string());
        }
        workout.record.file_references.push(path.to_string());
    }

    fn add_location(&mut self, attrs: &Attributes, events: &mut dyn Events) {
        let Some(workout) = self.workout.as_mut().filter(|workout| workout.in_route) else {
            return;
        };
        if let Some(note) = workout.notes.last_mut() {
            note.location_count += 1;
        }
        match location_point(attrs) {
            Ok(point) => workout.record.route.push(point),
            Err(example) => events.skip("inline WorkoutRoute points we could not store", Some(example), 1),
        }
    }

    fn add_lap(&mut self, attrs: &Attributes) {
        let Some(workout) = self.workout.as_mut() else {
            return;
        };
        if attrs.get("type").map(String::as_str) != Some("HKWorkoutEventTypeLap") {
            return;
        }
        let duration_s = attrs
            .get("duration")
            .and_then(|value| parse_f64(value))
            .filter(|duration| *duration > 0.0)
            .and_then(|duration| finite(round2(duration * 60.0)));
        let label = format!("lap {}", workout.record.laps.len() + 1);
        workout.record.laps.push(Lap { label, duration_s });
    }

    fn add_metadata(&mut self, attrs: &Attributes) {
        if let Some(workout) = self.workout.as_mut().filter(|workout| workout.in_route) {
            let mut entry = Map::new();
            if let Some(key) = attrs.get("key").filter(|value| !value.is_empty()) {
                entry.insert("key".into(), Value::String(key.clone()));
            }
            if let Some(value) = attrs.get("value").filter(|value| !value.is_empty()) {
                entry.insert("value".into(), Value::String(value.clone()));
            }
            if !entry.is_empty() {
                if let Some(note) = workout.notes.last_mut() {
                    note.metadata.push(Value::Object(entry));
                }
            }
            return;
        }
        if attrs.get("key").map(String::as_str) == Some("HKTimeZone") {
            if let Some(zone) = attrs.get("value").map(|value| value.trim()).filter(|value| !value.is_empty()) {
                if let Some(pending) = self.pending.as_mut() {
                    pending.set_zone(zone.to_string());
                }
                if let Some(workout) = self.workout.as_mut() {
                    workout.record.source_time_zone = Some(zone.to_string());
                }
            }
            return;
        }
        let Some(workout) = self.workout.as_mut() else {
            return;
        };
        let Some(value) = attrs.get("value").and_then(|value| parse_f64(value)).filter(|value| *value > 0.0) else {
            return;
        };
        let name = match attrs.get("key").map(String::as_str) {
            Some("HKAverageHeartRate") => "avg_hr_bpm",
            Some("HKMaximumHeartRate") => "max_hr_bpm",
            _ => return,
        };
        set(&mut workout.record.aggregates, name, finite(round2(value)));
        set(&mut workout.record.input_summaries, name, WrittenValue { input_value: value, input_unit: "bpm".into() });
    }

    fn add_beat(&mut self, attrs: &Attributes, events: &mut dyn Events) {
        let Some(Pending::Series(series)) = self.pending.as_mut() else {
            return;
        };
        let Some(beats) = series.beats.as_mut() else {
            return;
        };
        let bpm_text = attrs.get("bpm");
        let time = attrs.get("time").map(String::as_str).unwrap_or("");
        let Some(bpm) = bpm_text.and_then(|text| parse_f64(text)).filter(|bpm| *bpm > 0.0) else {
            let shown = bpm_text.map(String::as_str).unwrap_or("(none)");
            events.skip("heartbeat readings with an unusable rate", Some(format!("bpm \"{shown}\"")), 1);
            return;
        };
        let Some(offset_ms) = beat_offset_ms(&series.at, series.ends_at.as_deref(), time) else {
            let shown = if time.is_empty() { "(none)" } else { time };
            events.skip(
                "heartbeat readings we could not place in their reading's window",
                Some(format!("time \"{shown}\" in a window starting {}", series.at)),
                1,
            );
            return;
        };
        beats.push(Beat { offset_ms, interval_ms: 60000.0 / bpm });
    }
}

impl OpenWorkout {
    fn finish(self) -> WorkoutRecord {
        let mut record = self.record;
        record.route_fields = self
            .notes
            .into_iter()
            .map(|note| {
                let mut fields = note.fields;
                if let Some(path) = note.file_reference {
                    fields.insert("FileReference".into(), Value::String(path));
                }
                if !note.metadata.is_empty() {
                    fields.insert("MetadataEntry".into(), Value::Array(note.metadata));
                }
                Value::Object(fields)
            })
            .collect();
        record
    }
}

/// A number the helper could write as JSON. It wrote `null` for infinity.
fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

/// Insert or replace a named value, keeping the first position, as a
/// JavaScript object assignment does.
fn set<T>(list: &mut Vec<(String, T)>, name: &str, value: T) {
    match list.iter_mut().find(|(known, _)| known == name) {
        Some(entry) => entry.1 = value,
        None => list.push((name.to_string(), value)),
    }
}

fn push_summary(
    attrs: &Attributes,
    values: &mut Vec<ActivityValue>,
    (attribute, summary_type, standard_unit): (&str, &'static str, &'static str),
    energy_unit: Option<&str>,
    events: &mut dyn Events,
) {
    let Some(raw_text) = attrs.get(attribute) else {
        return;
    };
    let input_value = parse_f64(raw_text);
    let converted = match (input_value, energy_unit) {
        (Some(value), Some(unit)) => units::kilocalories(value, unit),
        (Some(value), None) => Some(value),
        (None, _) => None,
    };
    let Some((input_value, converted)) =
        input_value.zip(converted).filter(|(_, converted)| converted.is_finite() && *converted >= 0.0)
    else {
        events.skip("activity summary values we could not normalize", Some(format!("{attribute}=\"{raw_text}\"")), 1);
        return;
    };
    values.push(ActivityValue {
        summary_type,
        value: round2(converted),
        standard_unit,
        input_value,
        input_unit: energy_unit.map(str::to_string),
    });
}

/// `trim_start_matches`: every leading copy of the prefix is removed.
fn strip_all<'a>(text: &'a str, prefix: &str) -> &'a str {
    text.trim_start_matches(prefix)
}

fn identifier_of(raw_type: &str) -> String {
    strip_all(strip_all(strip_all(raw_type, "HKQuantityTypeIdentifier"), "HKCategoryTypeIdentifier"), "HKDataType")
        .to_string()
}

fn point_mapping(identifier: &str) -> Option<(PointMeasurementType, units::Converter)> {
    use PointMeasurementType as Q;
    Some(match identifier {
        "RestingHeartRate" => (Q::RestingHeartRate, units::per_minute),
        "WalkingHeartRateAverage" => (Q::WalkingHeartRate, units::per_minute),
        "HeartRateRecoveryOneMinute" => (Q::HrRecovery, units::per_minute),
        "BodyMass" => (Q::BodyWeight, units::kilograms),
        "LeanBodyMass" => (Q::LeanBodyMass, units::kilograms),
        "BodyFatPercentage" => (Q::BodyFatPercentage, units::percent),
        "Height" => (Q::Height, units::centimeters),
        "VO2Max" => (Q::Vo2Max, units::passthrough),
        "AppleSleepingWristTemperature" => (Q::WristTemperatureSleeping, units::celsius),
        "BodyTemperature" => (Q::BodyTemperature, units::celsius),
        "BloodPressureSystolic" => (Q::BloodPressureSystolic, units::millimetres_mercury),
        "BloodPressureDiastolic" => (Q::BloodPressureDiastolic, units::millimetres_mercury),
        _ => return None,
    })
}

fn series_mapping(identifier: &str) -> Option<(SeriesQuantity, units::Converter)> {
    use SeriesQuantity as Q;
    Some(match identifier {
        "HeartRate" => (Q::HeartRate, units::per_minute),
        "HeartRateVariabilitySDNN" => (Q::HrvSdnn, units::milliseconds),
        "RespiratoryRate" => (Q::RespiratoryRate, units::per_minute),
        "OxygenSaturation" => (Q::OxygenSaturation, units::percent),
        "StepCount" => (Q::Steps, units::count),
        "ActiveEnergyBurned" => (Q::ActiveEnergy, units::kilocalories),
        "BasalEnergyBurned" => (Q::BasalEnergy, units::kilocalories),
        "DistanceWalkingRunning" => (Q::DistanceWalkingRunning, units::metres),
        "DistanceCycling" => (Q::DistanceCycling, units::metres),
        "DistanceSwimming" => (Q::DistanceSwimming, units::metres),
        "RunningSpeed" => (Q::RunningSpeed, units::metres_per_second),
        "RunningPower" => (Q::RunningPower, units::watts),
        "RunningStrideLength" => (Q::RunningStrideLength, units::metres),
        "RunningVerticalOscillation" => (Q::RunningVerticalOscillation, units::centimeters),
        "RunningGroundContactTime" => (Q::RunningGroundContactTime, units::milliseconds),
        "PhysicalEffort" => (Q::PhysicalEffort, units::mets),
        "AppleExerciseTime" => (Q::ExerciseTime, units::minutes),
        "FlightsClimbed" => (Q::FlightsClimbed, units::count),
        "WalkingSpeed" => (Q::WalkingSpeed, units::metres_per_second),
        "WalkingStepLength" => (Q::WalkingStepLength, units::metres),
        "WalkingAsymmetryPercentage" => (Q::WalkingAsymmetryPercentage, units::percent),
        "TimeInDaylight" => (Q::TimeInDaylight, units::minutes),
        _ => return None,
    })
}

/// An inline route point, or the example that explains why it was left out.
fn location_point(attrs: &Attributes) -> std::result::Result<RoutePoint, String> {
    let date_raw = attrs.get("date").map(|value| value.trim()).unwrap_or("");
    let shown = if date_raw.is_empty() { "(none)" } else { date_raw };
    let Some(millis) = parse_apple_date(date_raw).as_deref().and_then(epoch_ms) else {
        return Err(format!("Location date \"{shown}\""));
    };
    let latitude = coordinate(attrs, "latitude", -90.0, 90.0)?;
    let longitude = coordinate(attrs, "longitude", -180.0, 180.0)?;
    let original = attrs.get("date").cloned().unwrap_or_default();
    let mut extra = Map::new();
    let mut keys: Vec<&String> =
        attrs.keys().filter(|key| !matches!(key.as_str(), "date" | "latitude" | "longitude")).collect();
    keys.sort();
    for key in keys {
        if let Some(value) = attrs.get(key).filter(|value| !value.is_empty()) {
            extra.insert(key.clone(), Value::String(value.clone()));
        }
    }
    Ok(RoutePoint {
        recorded_at: format_millis(millis),
        latitude_deg: latitude,
        longitude_deg: longitude,
        input_timestamp: Some(timestamp_receipt(&original, None)),
        supplied_fields: (!extra.is_empty()).then_some(extra),
    })
}

/// The helper's `format_millis`, used for inline route points.
fn format_millis(millis: i64) -> String {
    use chrono::{Datelike, Timelike};
    match DateTime::<Utc>::from_timestamp_millis(millis) {
        Some(value) => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            value.year(),
            value.month(),
            value.day(),
            value.hour(),
            value.minute(),
            value.second(),
            value.timestamp_subsec_millis()
        ),
        None => "1970-01-01T00:00:00.000Z".into(),
    }
}

fn coordinate(attrs: &Attributes, name: &str, low: f64, high: f64) -> std::result::Result<f64, String> {
    let raw = attrs.get(name).map(String::as_str).unwrap_or("(none)");
    let Some(value) = attrs.get(name).and_then(|text| parse_f64(text)) else {
        return Err(format!("Location {name} \"{raw}\""));
    };
    if !(low..=high).contains(&value) {
        return Err(format!("Location {name} \"{raw}\" is outside {low} to {high}"));
    }
    Ok(value)
}

fn sorted_attributes(attrs: &Attributes) -> Map<String, Value> {
    let mut keys: Vec<&String> = attrs.keys().collect();
    keys.sort();
    let mut fields = Map::new();
    for key in keys {
        if let Some(value) = attrs.get(key).filter(|value| !value.is_empty()) {
            fields.insert(key.clone(), Value::String(value.clone()));
        }
    }
    fields
}

/// `sourceName`, trimmed, or `Unknown`.
fn writer_name(attrs: &Attributes) -> String {
    let name = attrs.get("sourceName").map(|value| value.trim()).unwrap_or("");
    if name.is_empty() {
        "Unknown".into()
    } else {
        name.to_string()
    }
}

fn device_raw(attrs: &Attributes) -> String {
    attrs.get("device").cloned().unwrap_or_default()
}

/// The first ten characters when they are a `YYYY-MM-DD` shape, else empty.
fn on_day(raw: &str) -> String {
    let day: String = raw.chars().take(10).collect();
    if is_calendar_day(&day) {
        day
    } else {
        String::new()
    }
}

/// `YYYY-MM-DD` by shape only.
pub fn is_calendar_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

/// A finite number by Rust's float reading of the trimmed text.
pub fn parse_f64(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().filter(|value| value.is_finite())
}

/// The helper's rounding to two decimals.
pub fn round2(value: f64) -> f64 {
    helper_round(value * 100.0) / 100.0
}

fn round3(value: f64) -> f64 {
    helper_round(value * 1000.0) / 1000.0
}

fn digits(bytes: &[u8], start: usize, length: usize) -> bool {
    bytes.len() >= start + length && bytes[start..start + length].iter().all(u8::is_ascii_digit)
}

/// `2026-08-09 06:12:00 -0700` as `2026-08-09T06:12:00-07:00`. Complete ISO
/// 8601 text with `Z` or `±HH:MM` is kept as written. Anything else gives
/// nothing; a local time without an offset is not an instant.
pub fn parse_apple_date(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if is_full_iso(raw) {
        return Some(raw.to_string());
    }
    let bytes = raw.as_bytes();
    if bytes.len() < 19
        || !digits(bytes, 0, 4)
        || bytes[4] != b'-'
        || !digits(bytes, 5, 2)
        || bytes[7] != b'-'
        || !digits(bytes, 8, 2)
        || (bytes[10] != b' ' && bytes[10] != b'T')
        || !digits(bytes, 11, 2)
        || bytes[13] != b':'
        || !digits(bytes, 14, 2)
        || bytes[16] != b':'
        || !digits(bytes, 17, 2)
    {
        return None;
    }
    let rest = raw[19..].trim_start();
    let offset = rest.as_bytes();
    if offset.len() < 3 || (offset[0] != b'+' && offset[0] != b'-') || !digits(offset, 1, 2) {
        return None;
    }
    let minutes = if offset.len() == 3 {
        "00"
    } else if offset.len() == 5 && digits(offset, 3, 2) {
        &rest[3..5]
    } else if offset.len() == 6 && offset[3] == b':' && digits(offset, 4, 2) {
        &rest[4..6]
    } else {
        return None;
    };
    Some(format!(
        "{}-{}-{}T{}:{}:{}{}:{}",
        &raw[0..4],
        &raw[5..7],
        &raw[8..10],
        &raw[11..13],
        &raw[14..16],
        &raw[17..19],
        &rest[0..3],
        minutes
    ))
}

fn is_full_iso(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() < 20
        || !digits(bytes, 0, 4)
        || bytes[4] != b'-'
        || !digits(bytes, 5, 2)
        || bytes[7] != b'-'
        || !digits(bytes, 8, 2)
        || bytes[10] != b'T'
        || !digits(bytes, 11, 2)
        || bytes[13] != b':'
        || !digits(bytes, 14, 2)
        || bytes[16] != b':'
        || !digits(bytes, 17, 2)
    {
        return false;
    }
    let mut index = 19;
    if bytes[index] == b'.' {
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
    matches!(bytes[index], b'+' | b'-')
        && index + 6 == bytes.len()
        && digits(bytes, index + 1, 2)
        && bytes[index + 3] == b':'
        && digits(bytes, index + 4, 2)
}

/// Milliseconds by chrono's RFC 3339 reading, as the helper read them.
pub fn epoch_ms(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso).ok().map(|time| time.timestamp_millis())
}

/// chrono's `%Y-%m-%dT%H:%M:%S%.3fZ`; the epoch for an instant chrono cannot hold.
pub fn format_utc(millis: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(millis)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// The trailing `Z`, `±HH:MM`, or `±HHMM` of trimmed text, as written.
pub fn trailing_offset(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    if bytes.last().is_some_and(|byte| *byte == b'Z' || *byte == b'z') {
        return Some(&raw[raw.len() - 1..]);
    }
    if raw.len() >= 6 {
        let tail = &bytes[raw.len() - 6..];
        if (tail[0] == b'+' || tail[0] == b'-') && digits(tail, 1, 2) && tail[3] == b':' && digits(tail, 4, 2) {
            return Some(&raw[raw.len() - 6..]);
        }
    }
    if raw.len() >= 5 {
        let tail = &bytes[raw.len() - 5..];
        if (tail[0] == b'+' || tail[0] == b'-') && digits(tail, 1, 4) {
            return Some(&raw[raw.len() - 5..]);
        }
    }
    None
}

/// The receipt the helper wrote for Apple time text, with the timezone an
/// `HKTimeZone` metadata entry named.
pub fn timestamp_receipt(raw: &str, zone: Option<&str>) -> InputTimestampReceipt {
    let offset = trailing_offset(raw.trim());
    InputTimestampReceipt {
        timestamp_text: Some(raw.to_string()),
        utc_offset_text: offset.map(str::to_string),
        utc_offset_meaning: offset.map(|_| UtcOffsetMeaning::MeaningNotDocumented),
        source_time_zone: zone.map(str::to_string),
        source_time_zone_meaning: zone.map(|_| SourceTimeZoneMeaning::AppleHealthObjectCreationTimeZone),
        timestamp_basis: Some(InputTimestampBasis::SourceOffsetTimestamp),
        timestamp_missing_reason: None,
        utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
    }
}

/// An instant written with the offset of a reference time, always with
/// milliseconds, as the helper wrote beat times.
pub fn at_offset_of(reference: &str, instant_ms: f64) -> String {
    let rounded = helper_round(instant_ms) as i64;
    let bytes = reference.as_bytes();
    if reference.len() >= 6 {
        let tail = &bytes[reference.len() - 6..];
        if (tail[0] == b'+' || tail[0] == b'-') && digits(tail, 1, 2) && tail[3] == b':' && digits(tail, 4, 2) {
            let offset = &reference[reference.len() - 6..];
            let sign = if tail[0] == b'-' { -1 } else { 1 };
            let hours: i64 = offset[1..3].parse().unwrap_or(0);
            let minutes: i64 = offset[4..6].parse().unwrap_or(0);
            let shifted = format_utc(rounded + sign * (hours * 60 + minutes) * 60_000);
            return format!("{}{offset}", &shifted[..23]);
        }
    }
    format_utc(rounded)
}

fn clock_ms(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    let index = bytes.iter().position(|byte| *byte == b'T')? + 1;
    if index + 8 > bytes.len()
        || !(digits(bytes, index, 2)
            && bytes[index + 2] == b':'
            && digits(bytes, index + 3, 2)
            && bytes[index + 5] == b':'
            && digits(bytes, index + 6, 2))
    {
        return None;
    }
    let hour = two(bytes, index);
    let minute = two(bytes, index + 3);
    let second = two(bytes, index + 6);
    let fraction = fraction_ms(bytes, index + 8, b".");
    Some((hour * 3600 + minute * 60 + second) * 1000 + fraction)
}

fn two(bytes: &[u8], index: usize) -> i64 {
    i64::from(bytes[index] - b'0') * 10 + i64::from(bytes[index + 1] - b'0')
}

/// Up to three fraction digits after a separator, as milliseconds.
fn fraction_ms(bytes: &[u8], at: usize, separators: &[u8]) -> i64 {
    if at >= bytes.len() || !separators.contains(&bytes[at]) {
        return 0;
    }
    let start = at + 1;
    let mut cursor = start;
    while cursor < bytes.len() && cursor - start < 3 && bytes[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == start {
        return 0;
    }
    let mut value: i64 = bytes[start..cursor].iter().fold(0, |total, digit| total * 10 + i64::from(digit - b'0'));
    for _ in cursor - start..3 {
        value *= 10;
    }
    value
}

/// Where a beat falls in its record's window, in milliseconds from the start.
/// Only the minute and second are read: a whole-hour disagreement between the
/// phone's clock and the record cannot move a beat out of its minute.
pub fn beat_offset_ms(recorded_at: &str, ends_at: Option<&str>, time_of_day: &str) -> Option<i64> {
    let beat_within_hour = beat_within_hour(time_of_day)?;
    let start_ms = clock_ms(recorded_at)?;
    let duration_ms = ends_at.and_then(clock_ms).map(|end| (end - start_ms + DAY_MS) % DAY_MS).unwrap_or(0);
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
    (0..bytes.len()).filter(|index| bytes[*index].is_ascii_digit()).find_map(|index| beat_at(bytes, index))
}

fn beat_at(bytes: &[u8], index: usize) -> Option<i64> {
    for length in [2_usize, 1] {
        if !digits(bytes, index, length) {
            continue;
        }
        let colon = index + length;
        if colon + 5 >= bytes.len() || bytes[colon] != b':' || bytes[colon + 3] != b':' {
            continue;
        }
        if !(digits(bytes, colon + 1, 2) && digits(bytes, colon + 4, 2)) {
            continue;
        }
        let minute = two(bytes, colon + 1);
        let second = two(bytes, colon + 4);
        let fraction = fraction_ms(bytes, colon + 6, b".,");
        return Some(minute * 60_000 + second * 1000 + fraction);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "2026-08-09T06:12:00-07:00";
    const END: &str = "2026-08-09T06:13:01-07:00";

    #[test]
    fn apple_dates_keep_their_offset() {
        assert_eq!(parse_apple_date("2026-08-09 06:12:00 -0700").as_deref(), Some("2026-08-09T06:12:00-07:00"));
        assert_eq!(parse_apple_date("2026-08-09 06:12:00 +05").as_deref(), Some("2026-08-09T06:12:00+05:00"));
        assert_eq!(parse_apple_date("2026-08-09T06:12:00.5Z").as_deref(), Some("2026-08-09T06:12:00.5Z"));
        assert_eq!(parse_apple_date("9 Aug 2026, 6:12am"), None);
        assert_eq!(parse_apple_date("2026-08-09 06:12:00"), None);
    }

    #[test]
    fn a_beat_is_placed_by_minute_and_second_on_any_clock() {
        assert_eq!(beat_offset_ms(START, Some(END), "6:12:30.50"), Some(30_500));
        assert_eq!(beat_offset_ms(START, Some(END), "06:12:30.50"), Some(30_500));
        assert_eq!(beat_offset_ms(START, Some(END), "6:12:30.50 AM"), Some(30_500));
        assert_eq!(beat_offset_ms(START, Some(END), "오전 6:12:30.50"), Some(30_500));
        assert_eq!(beat_offset_ms(START, Some(END), "6:12:30.50 a.m."), Some(30_500));
        assert_eq!(beat_offset_ms(START, Some(END), "6:12:30,50"), Some(30_500));
        let (afternoon, afternoon_end) = ("2017-10-31T13:40:43+00:00", "2017-10-31T13:41:54+00:00");
        assert_eq!(beat_offset_ms(afternoon, Some(afternoon_end), "1:40:45.22 PM"), Some(2_220));
        assert_eq!(beat_offset_ms(afternoon, Some(afternoon_end), "13:40:45.22"), Some(2_220));
        assert_eq!(beat_offset_ms(afternoon, Some(afternoon_end), "6:12:30.50"), None);
        assert_eq!(
            beat_offset_ms("2020-11-15T23:33:17-05:00", Some("2020-11-15T23:34:18-05:00"), "10:33:19.09 PM"),
            Some(2_090)
        );
        for hour in ["06", "07", "05", "18", "00"] {
            assert_eq!(beat_offset_ms(START, Some(END), &format!("{hour}:12:30.50")), Some(30_500));
        }
        assert_eq!(
            beat_offset_ms("2026-08-09T12:00:00-07:00", Some("2026-08-09T12:01:01-07:00"), "12:00:30.00"),
            Some(30_000)
        );
        assert_eq!(
            beat_offset_ms("2026-08-09T00:00:00-07:00", Some("2026-08-09T00:01:01-07:00"), "12:00:30.00 AM"),
            Some(30_000)
        );
        assert_eq!(
            beat_offset_ms("2026-08-09T23:59:40-07:00", Some("2026-08-10T00:00:41-07:00"), "12:00:15.00 AM"),
            Some(35_000)
        );
        for nowhere in ["quarter past six", "99:99:99", "6:45:00.00"] {
            assert_eq!(beat_offset_ms(START, Some(END), nowhere), None, "{nowhere}");
        }
        assert_eq!(beat_offset_ms(START, None, "6:12:30.50"), Some(30_500));
    }

    #[test]
    fn beat_times_keep_the_record_offset_and_milliseconds() {
        let start_ms = epoch_ms(START).unwrap() as f64;
        assert_eq!(at_offset_of(START, start_ms + 220.0), "2026-08-09T06:12:00.220-07:00");
        assert_eq!(at_offset_of(START, start_ms + 219.5), "2026-08-09T06:12:00.220-07:00");
        assert_eq!(at_offset_of("2026-08-09T13:12:00Z", start_ms), "2026-08-09T13:12:00.000Z");
    }

    #[test]
    fn receipts_name_the_offset_as_written() {
        let receipt = timestamp_receipt("2026-08-09 06:12:00 -0700", Some("America/Los_Angeles"));
        assert_eq!(receipt.utc_offset_text.as_deref(), Some("-0700"));
        assert_eq!(receipt.source_time_zone.as_deref(), Some("America/Los_Angeles"));
        let none = timestamp_receipt("whenever", None);
        assert_eq!(none.utc_offset_missing_reason, Some(MissingReason::NotProvidedBySource));
    }
}
