//! Apple Health exports: `export.xml`, its workout routes, and its ECGs.
//!
//! The reader follows 2.1.0 (`crates/ath-core/src/apple.rs` with
//! `src/import/apple.ts`, `routes.ts`, `ecg.ts`, `src/rpeaks.ts`, `src/hrv.ts`,
//! and the Apple rules of `src/db/provider-import.ts`). Each writer named in the
//! export is its own source, and a value typed into the Health app belongs to
//! the manual source.
//!
//! - Point measurements, such as resting heart rate and body weight, are sent
//!   as they are read.
//! - Series samples, such as heart rate and SDNN, are stored one record per
//!   sample. They are listed by quantity and writer, then by day, then by time,
//!   as 2.1.0 listed them, so they wait in a scratch file until the export has
//!   been read.
//! - SDNN stays `hrv_sdnn`. The beats listed under an SDNN sample become
//!   `hrv_beats` samples, and RMSSD calculated from them becomes an `hrv_rmssd`
//!   measurement. RMSSD and SDNN never share a record.
//! - Sleep stage records are joined into nights per writer: a gap over three
//!   hours starts a new night. Night totals are calculated on import.
//! - Workouts keep their laps, inline route points, and the route files a
//!   `WorkoutRoute` names. Every route file is also summarized into kilometre
//!   splits and climbing for the workout it overlaps; those values are marked
//!   `route_summaries: calculated_on_import`.
//! - ECG recordings in sinus rhythm give `ecg_beats` samples and an RMSSD under
//!   their own source, because electrical and optical beats are different
//!   measurements.
//! - Activity summaries become provider summaries for Apple's activity day.
//!
//! Every omitted object is counted under a reason with its first example.

mod ecg;
mod files;
pub mod hrv;
pub mod routes;
pub mod rpeaks;
mod scratch;
pub mod trig;
mod writers;
pub mod xml;

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::*;

use super::candidate::{
    missing_timestamp_receipt, offset_text, source_reported_summaries, source_value, value_basis, value_field_basis,
    Aggregate, BasisFacts, Candidate, KeyPart, MatchKey, Section, SourceInput,
};
use super::detect::DetectedExport;
use super::instant::{normalized_utc, parse_instant};
use super::provider::{CandidateSink, ProviderReader};
use scratch::Scratch;
use writers::{Device, Spaces, Writers};
use xml::{ActivityRecord, PointRecord, SeriesRecord, SleepFragment, WorkoutRecord};

pub use ecg::{beats_from_ecg, is_usable_rhythm, parse_ecg_csv, EcgBeats, EcgProblem, EcgRecording};
pub use writers::{parse_device, vendor_of, writer_slug};

/// The Apple Health route.
pub struct AppleReader;

impl ProviderReader for AppleReader {
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()> {
        let source = files::locate_xml(export)?;
        let mut import = Import::new(sink)?;
        files::with_xml(export, &source, |reader| xml::read(reader, &mut import))?;
        let listing = files::Listing::read(export)?;
        import.workouts(export, &listing)?;
        import.nights()?;
        import.ecgs(export, &listing)?;
        import.finish()
    }
}

/// Where a record sits among the records 2.1.0 planned, for reporting the
/// problem 2.1.0 would have reported first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Points,
    Workouts,
    Nights,
    EcgReadings,
    BeatReadings,
    Summaries,
    Samples,
}

/// Problems found in values. 2.1.0 read the whole export before it turned
/// values into records, so a broken file was reported before a broken value,
/// and an unreadable time before a record that fails the format.
#[derive(Default)]
struct Deferred {
    time: Option<(Stage, Error)>,
    invalid: Option<(Stage, Error)>,
}

impl Deferred {
    fn keep(slot: &mut Option<(Stage, Error)>, stage: Stage, error: Error) {
        if slot.as_ref().is_none_or(|(kept, _)| stage < *kept) {
            *slot = Some((stage, error));
        }
    }
}

/// Which list a sample belongs to: ECG beats first, then HRV beats, then the
/// other series, as 2.1.0 listed them.
const PHASE_ECG: i64 = 0;
const PHASE_BEATS: i64 = 1;
const PHASE_SERIES: i64 = 2;

/// What the route phase needs to know about a workout.
struct WorkoutMeta {
    seq: i64,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    has_inline_route: bool,
    has_laps: bool,
    file_references: Vec<String>,
    /// Route summary results: elevation gain and the summary that gives splits.
    elevation_gain_m: Option<f64>,
    splits_from: Option<usize>,
    route_summaries_calculated: bool,
}

/// A beat-derived RMSSD that waits until ECG readings have been planned.
#[derive(serde::Serialize, serde::Deserialize)]
struct BeatReading {
    source_id: String,
    recorded_at: String,
    input_raw: String,
    zone: Option<String>,
    rmssd_ms: f64,
    window_s: f64,
    n_beats: usize,
    n_dropped: usize,
}

struct Import<'s, 'a> {
    sink: &'s mut CandidateSink<'a>,
    scratch: Scratch,
    writers: Writers,
    deferred: Deferred,
    sample_seq: i64,
    series_buckets: HashMap<(SeriesQuantity, String), i64>,
    beat_buckets: HashMap<(String, String), i64>,
    days: HashMap<(i64, i64, String), i64>,
    spans: HashSet<(i64, i64, i64)>,
    fragment_groups: HashMap<String, i64>,
    fragment_seq: i64,
    workouts: Vec<WorkoutMeta>,
    beat_readings: i64,
    short_windows: u64,
    route_summaries: Vec<routes::RouteSummary>,
}

impl<'s, 'a> Import<'s, 'a> {
    fn new(sink: &'s mut CandidateSink<'a>) -> Result<Self> {
        Ok(Import {
            sink,
            scratch: Scratch::create()?,
            writers: Writers::default(),
            deferred: Deferred::default(),
            sample_seq: 0,
            series_buckets: HashMap::new(),
            beat_buckets: HashMap::new(),
            days: HashMap::new(),
            spans: HashSet::new(),
            fragment_groups: HashMap::new(),
            fragment_seq: 0,
            workouts: Vec::new(),
            beat_readings: 0,
            short_windows: 0,
            route_summaries: Vec::new(),
        })
    }

    fn basis(&self, record_type: RecordType, quantity: Option<&str>, derived: bool, manual: bool) -> FieldBasis {
        value_field_basis(value_basis(BasisFacts {
            data_route: &self.sink.route().data_route,
            record_type,
            quantity,
            derived,
            manual,
        }))
    }

    /// A stored UTC instant, or nothing after keeping the problem for later.
    fn instant(&mut self, stage: Stage, text: &str) -> Option<String> {
        match normalized_utc(text) {
            Ok(instant) => Some(instant),
            Err(error) => {
                Deferred::keep(&mut self.deferred.time, stage, error);
                None
            }
        }
    }

    fn add(&mut self, stage: Stage, candidate: Candidate) -> Result<()> {
        match self.sink.add(candidate) {
            Err(Error::Invalid(error)) => {
                Deferred::keep(&mut self.deferred.invalid, stage, Error::Invalid(error));
                Ok(())
            }
            other => other,
        }
    }

    fn day_index(&mut self, phase: i64, bucket: i64, day: &str) -> i64 {
        let next = self.days.len() as i64;
        *self.days.entry((phase, bucket, day.to_string())).or_insert(next)
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_sample(
        &mut self,
        group: (i64, i64, i64),
        source_id: &str,
        manual: bool,
        quantity: SeriesQuantity,
        at: &str,
        value: f64,
        duration_ms: i64,
        input: Option<(&str, f64, Option<&str>, Option<&str>)>,
    ) -> Result<()> {
        let Some(instant_ms) = parse_instant(at) else {
            let error = Error::other(format!("Provider timestamp is not an absolute instant: {at}"));
            Deferred::keep(&mut self.deferred.time, Stage::Samples, error);
            return Ok(());
        };
        if duration_ms > 0 {
            self.spans.insert(group);
        }
        let (phase, bucket, day) = group;
        let seq = self.sample_seq;
        self.sample_seq += 1;
        self.scratch
            .connection()
            .prepare_cached(
                "INSERT INTO samples (phase, bucket, day, instant_ms, seq, source_id, manual, quantity, at, value,
                    duration_ms, input_raw, input_value, input_unit, zone)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?
            .execute(params![
                phase,
                bucket,
                day,
                instant_ms,
                seq,
                source_id,
                manual,
                quantity.as_str(),
                at,
                value,
                duration_ms,
                input.map(|input| input.0),
                input.map(|input| input.1),
                input.and_then(|input| input.2),
                input.and_then(|input| input.3),
            ])?;
        Ok(())
    }

    // ---- Workouts ---------------------------------------------------------

    fn workouts(&mut self, export: &DetectedExport, listing: &files::Listing) -> Result<()> {
        self.scratch.settle()?;
        self.scratch.connection().execute_batch("BEGIN")?;
        // Every route file is read in listing order, as 2.1.0 read them all
        // before using any. A file a workout names is kept for that workout.
        let wanted: HashSet<String> = self
            .workouts
            .iter()
            .filter(|workout| !workout.has_inline_route)
            .flat_map(|workout| workout.file_references.iter().map(|reference| listing.wanted_name(reference)))
            .collect();
        let mut first_by_name: HashMap<String, String> = HashMap::new();
        let mut summaries: Vec<(String, Option<routes::RouteSummary>)> = Vec::new();
        for name in listing.route_files() {
            let text = files::read_text(export, name)?;
            let lower = name.replace('\\', "/").to_lowercase();
            if !first_by_name.contains_key(&lower) {
                first_by_name.insert(lower.clone(), name.to_string());
                if wanted.contains(&lower) {
                    self.scratch
                        .connection()
                        .prepare_cached("INSERT INTO route_texts (name, body) VALUES (?1, ?2)")?
                        .execute(params![name, text])?;
                }
            }
            summaries.push((name.to_string(), routes::summarize_route(routes::parse_gpx(&text))));
        }

        // Route files a WorkoutRoute names. A workout with inline points
        // already has its route, and one file is stored on one workout.
        let mut stored: HashSet<String> = HashSet::new();
        let mut handled: HashSet<String> = HashSet::new();
        for index in 0..self.workouts.len() {
            if self.workouts[index].has_inline_route {
                continue;
            }
            for reference in self.workouts[index].file_references.clone() {
                let shown = reference.replace('\\', "/").trim_start_matches('/').to_string();
                let Some(name) = first_by_name.get(&listing.wanted_name(&reference)).cloned() else {
                    self.sink.skip("workout route file named by a WorkoutRoute was not in the export", &shown);
                    continue;
                };
                if stored.contains(&name) {
                    self.sink.skip("workout route file is already stored on another workout", &shown);
                    handled.insert(name);
                    continue;
                }
                let text: String = self.scratch.connection().query_row(
                    "SELECT body FROM route_texts WHERE name = ?1",
                    [&name],
                    |row| row.get(0),
                )?;
                let (points, skipped) = routes::route_from_gpx(&text);
                if points.is_empty() {
                    self.sink.skip("workout route file had no usable points", &shown);
                    handled.insert(name);
                    continue;
                }
                let workout = &self.workouts[index];
                if !routes::overlaps(workout.start_ms, workout.end_ms, &points) {
                    self.sink.skip("workout route file is outside the workout's time", &shown);
                    handled.insert(name);
                    continue;
                }
                for example in skipped {
                    self.sink.skip("GPX route points we could not store", &format!("{shown}: {example}"));
                }
                let body = serde_json::to_string(&points).map_err(|error| Error::other(error.to_string()))?;
                self.scratch
                    .connection()
                    .prepare_cached("INSERT INTO attached_routes (workout, body) VALUES (?1, ?2)")?
                    .execute(params![workout.seq, body])?;
                stored.insert(name.clone());
                handled.insert(name);
            }
        }
        self.scratch.connection().execute("DELETE FROM route_texts", [])?;

        // Splits and climbing from every route file, for the first route that
        // overlaps each workout. Laps the watch recorded are kept over splits.
        let mut present: Vec<(String, routes::RouteSummary)> = Vec::new();
        for (name, summary) in summaries {
            match summary {
                Some(summary) => present.push((name, summary)),
                None if !handled.contains(&name) => self.sink.skip("workout routes with too few track points", &name),
                None => {}
            }
        }
        let mut matched: HashSet<usize> = HashSet::new();
        for workout in &mut self.workouts {
            let (Some(start_ms), Some(end_ms)) = (workout.start_ms, workout.end_ms) else {
                continue;
            };
            let Some(found) = present.iter().position(|(_, route)| route.start_ms < end_ms && route.end_ms > start_ms)
            else {
                continue;
            };
            matched.insert(found);
            let route = &present[found].1;
            if route.elevation_gain_m > 0.0 {
                workout.elevation_gain_m = Some(route.elevation_gain_m);
                workout.route_summaries_calculated = true;
            }
            if !workout.has_laps && !route.splits.is_empty() {
                workout.splits_from = Some(found);
                workout.route_summaries_calculated = true;
            }
        }
        let unmatched = present
            .iter()
            .enumerate()
            .filter(|(index, (name, _))| !matched.contains(index) && !handled.contains(name))
            .count();
        if unmatched > 0 {
            self.sink.skip_count("workout routes with no workout in the same window", unmatched as u64);
        }
        self.route_summaries = present.into_iter().map(|(_, summary)| summary).collect();
        self.scratch.settle()?;

        for index in 0..self.workouts.len() {
            self.emit_workout(index)?;
        }
        Ok(())
    }

    fn emit_workout(&mut self, index: usize) -> Result<()> {
        let seq = self.workouts[index].seq;
        let body: String =
            self.scratch.connection().query_row("SELECT body FROM workouts WHERE seq = ?1", [seq], |row| row.get(0))?;
        let stored: StoredWorkout = serde_json::from_str(&body).map_err(|error| Error::other(error.to_string()))?;
        let StoredWorkout { record, source_id, manual } = stored;
        let mut route = record.route;
        let attached: Vec<String> = self
            .scratch
            .connection()
            .prepare_cached("SELECT body FROM attached_routes WHERE workout = ?1 ORDER BY seq")?
            .query_map([seq], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for chunk in attached {
            let points: Vec<RoutePoint> =
                serde_json::from_str(&chunk).map_err(|error| Error::other(error.to_string()))?;
            route.extend(points);
        }
        let (Some(start_at), Some(end_at)) =
            (self.instant(Stage::Workouts, &record.start), self.instant(Stage::Workouts, &record.end))
        else {
            return Ok(());
        };
        for point in &mut route {
            match self.instant(Stage::Workouts, &point.recorded_at) {
                Some(instant) => point.recorded_at = instant,
                None => return Ok(()),
            }
        }
        let meta = &self.workouts[index];
        let mut aggregates: Vec<Aggregate> = record
            .aggregates
            .iter()
            .filter_map(|(name, value)| {
                let value = (*value)?;
                let input = record
                    .input_summaries
                    .iter()
                    .find(|(known, _)| known == name)
                    .map(|(_, written)| SourceInput::written(written.input_value, written.input_unit.clone()));
                Some(Aggregate { name: name.clone(), value, input })
            })
            .collect();
        if let Some(gain) = meta.elevation_gain_m {
            aggregates.push(Aggregate { name: "elevation_gain_m".into(), value: gain, input: None });
        }
        let segment = |label: &str, values: Vec<(&str, f64)>| WorkoutSessionSegment {
            label: label.to_string(),
            start_offset_ms: None,
            end_offset_ms: None,
            source_reported_summaries: source_reported_summaries(
                &values
                    .into_iter()
                    .map(|(name, value)| Aggregate { name: name.into(), value, input: None })
                    .collect::<Vec<_>>(),
            ),
        };
        let segments = if !record.laps.is_empty() {
            Some(
                record
                    .laps
                    .iter()
                    .map(|lap| {
                        segment(&lap.label, lap.duration_s.map(|value| ("duration_s", value)).into_iter().collect())
                    })
                    .collect(),
            )
        } else {
            meta.splits_from.map(|found| {
                self.route_summaries[found]
                    .splits
                    .iter()
                    .map(|split| {
                        segment(&split.label, vec![("duration_s", split.duration_s), ("distance_m", split.distance_m)])
                    })
                    .collect()
            })
        };
        let mut field_basis = self.basis(RecordType::WorkoutSession, None, false, manual);
        if !record.activity.is_empty() {
            field_basis.activity_type = Some(ActivityTypeBasis::ProviderLabel);
        }
        if meta.route_summaries_calculated {
            field_basis.route_summaries = Some(RouteSummariesBasis::CalculatedOnImport);
        }
        let supplied_fields = (!record.route_fields.is_empty()).then(|| {
            let mut fields = json::Map::new();
            fields.insert("WorkoutRoute".into(), Value::Array(record.route_fields.clone()));
            fields
        });
        let zone = record.source_time_zone.as_deref();
        let data = WorkoutSessionData {
            source_id,
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields,
            field_basis: Some(field_basis),
            start_at: start_at.clone(),
            end_at,
            input_start_timestamp: xml::timestamp_receipt(&record.input_start, zone),
            input_end_timestamp: xml::timestamp_receipt(&record.input_end, zone),
            activity_type: (!record.activity.is_empty()).then(|| record.activity.clone()),
            name: None,
            description: None,
            text_result: None,
            maximum_heart_rate_bpm: None,
            resting_heart_rate_bpm: None,
            heart_rate_samples: None,
            sets: None,
            route: (!route.is_empty()).then_some(route),
            source_reported_summaries: source_reported_summaries(&aggregates),
            segments,
        };
        self.add(
            Stage::Workouts,
            Candidate::new(
                RecordData::WorkoutSession(data),
                MatchKey::session(RecordType::WorkoutSession, &start_at),
                Section::Record,
            ),
        )
    }

    // ---- Sleep --------------------------------------------------------------

    /// Join each writer's sleep fragments into nights and send them.
    fn nights(&mut self) -> Result<()> {
        self.scratch.settle()?;
        let mut open: Option<(i64, Night)> = None;
        let mut after = (-1_i64, i64::MIN, -1_i64);
        loop {
            let page = self.fragment_page(after)?;
            let Some(last) = page.last() else {
                break;
            };
            after = (last.0, last.1.start_ms, last.2);
            let mut finished = Vec::new();
            for (group, fragment, _) in page {
                match open.as_mut() {
                    // A gap over three hours starts a new night.
                    Some((current, night))
                        if *current == group && fragment.start_ms - night.cluster_end <= SLEEP_GAP_MS =>
                    {
                        night.extend(fragment);
                    }
                    _ => {
                        if let Some((_, night)) = open.take() {
                            finished.push(night);
                        }
                        open = Some((group, Night::start(fragment)));
                    }
                }
            }
            for night in finished {
                self.emit_night(night)?;
            }
        }
        if let Some((_, night)) = open.take() {
            self.emit_night(night)?;
        }
        Ok(())
    }

    /// Fragments after a position, by writer group, start, and arrival.
    fn fragment_page(&self, after: (i64, i64, i64)) -> Result<Vec<(i64, SleepFragment, i64)>> {
        let mut statement = self.scratch.connection().prepare_cached(
            "SELECT grp, seq, body FROM fragments WHERE (grp, start_ms, seq) > (?1, ?2, ?3)
              ORDER BY grp, start_ms, seq LIMIT ?4",
        )?;
        let rows = statement
            .query_map(params![after.0, after.1, after.2, crate::input_policy::IMPORT_BATCH_RECORDS as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(group, seq, body)| {
                let fragment: SleepFragment =
                    serde_json::from_str(&body).map_err(|error| Error::other(error.to_string()))?;
                Ok((group, fragment, seq))
            })
            .collect()
    }

    fn emit_night(&mut self, night: Night) -> Result<()> {
        let Some(aggregates) = night.aggregates() else {
            return Ok(());
        };
        let writer = self.writers.resolve(self.sink, &night.start.writer, &night.device_raw, "");
        if let Some(device) = parse_device(&night.device_raw, Spaces::JavaScript) {
            self.writers.accept(&(writer.source_id.clone(), device));
        }
        let (Some(start_at), Some(end_at)) =
            (self.instant(Stage::Nights, &night.start.start), self.instant(Stage::Nights, &night.end.end))
        else {
            return Ok(());
        };
        let data = SleepSessionData {
            source_id: writer.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::SleepSession, None, false, writer.manual)),
            start_at: start_at.clone(),
            end_at,
            input_start_timestamp: xml::timestamp_receipt(
                &night.start.input_start,
                night.start.source_time_zone.as_deref(),
            ),
            input_end_timestamp: xml::timestamp_receipt(&night.end.input_end, night.end.source_time_zone.as_deref()),
            source_reported_summaries: source_reported_summaries(&aggregates),
            asleep_seconds: None,
            in_bed_seconds: None,
            interruptions: None,
            stage_minutes: None,
        };
        self.add(
            Stage::Nights,
            Candidate::new(
                RecordData::SleepSession(data),
                MatchKey::session(RecordType::SleepSession, &start_at),
                Section::Record,
            ),
        )
    }

    // ---- ECG ----------------------------------------------------------------

    fn ecgs(&mut self, export: &DetectedExport, listing: &files::Listing) -> Result<()> {
        let names = listing.ecg_files();
        if names.is_empty() {
            return Ok(());
        }
        let source_id = self.sink.export_source("Apple Watch", "apple", Some("ecg"));
        self.scratch.connection().execute_batch("BEGIN")?;
        for name in names {
            let text = files::read_text(export, name)?;
            let shown = files::base_name(name);
            let recording = match parse_ecg_csv(&text) {
                Ok(recording) => recording,
                Err(problem) => {
                    self.sink
                        .skip("ECG recordings we could not read", &format!("{shown}: {}", problem_detail(&problem)));
                    continue;
                }
            };
            let beats = match beats_from_ecg(&recording) {
                Ok(beats) => beats,
                Err(problem) => {
                    let reason = match problem {
                        EcgProblem::NotSinus(_) => {
                            "ECG recordings not in sinus rhythm, where variability does not describe recovery"
                        }
                        _ => "ECG recordings with too few detectable beats",
                    };
                    self.sink.skip(reason, &format!("{shown}: {}", problem_detail(&problem)));
                    continue;
                }
            };
            let day: String = beats.recorded_at.chars().take(10).collect();
            let day_index = self.day_index(PHASE_ECG, 0, &day);
            // 2.1.0 stopped here with JavaScript's own message for a date it
            // could not place in time.
            let Some(start_ms) = parse_instant(&beats.recorded_at) else {
                return Err(Error::other("Invalid time value"));
            };
            for (offset, interval) in beats.offsets_ms.iter().skip(1).zip(&beats.intervals_ms) {
                let at = ecg_at_offset_of(&beats.recorded_at, start_ms as f64 + offset)?;
                let value = js_round(interval * 10.0) / 10.0;
                self.stage_sample(
                    (PHASE_ECG, 0, day_index),
                    &source_id,
                    false,
                    SeriesQuantity::EcgBeats,
                    &at,
                    value,
                    0,
                    None,
                )?;
            }
            let Some(rmssd) = hrv::rmssd_from_intervals(&beats.intervals_ms) else {
                self.sink.skip("ECG recordings with too few usable intervals for RMSSD", shown);
                continue;
            };
            let Some(recorded_at) = self.instant(Stage::EcgReadings, &beats.recorded_at) else {
                continue;
            };
            let offset = routes::trailing_offset_loose(crate::text::js_trim(&beats.recorded_at));
            let receipt = InputTimestampReceipt {
                timestamp_text: Some(beats.recorded_at.clone()),
                utc_offset_text: offset.map(str::to_string),
                utc_offset_meaning: offset.map(|_| UtcOffsetMeaning::MeaningNotDocumented),
                source_time_zone: None,
                source_time_zone_meaning: None,
                timestamp_basis: Some(InputTimestampBasis::SourceOffsetTimestamp),
                timestamp_missing_reason: None,
                utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
            };
            let quality = (beats.quality > 0.0).then_some(beats.quality);
            let candidate = self.rmssd_reading(&source_id, recorded_at, receipt, "ecg_beats", rmssd, quality);
            self.add(Stage::EcgReadings, candidate)?;
        }
        self.scratch.settle()
    }

    fn rmssd_reading(
        &self,
        source_id: &str,
        recorded_at: String,
        receipt: InputTimestampReceipt,
        from: &str,
        rmssd: hrv::Rmssd,
        quality: Option<f64>,
    ) -> Candidate {
        let not_applicable = SourceInput {
            input_value_missing_reason: Some(MissingReason::NotApplicable),
            input_unit_missing_reason: Some(MissingReason::NotApplicable),
            ..SourceInput::default()
        };
        let data = PointMeasurementData {
            source_id: source_id.to_string(),
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::PointMeasurement, Some("hrv_rmssd"), true, false)),
            quantity: PointMeasurementType::HrvRmssd,
            recorded_at: Some(recorded_at.clone()),
            input_timestamp: receipt,
            value: source_value(rmssd.rmssd_ms, "ms", Some(&not_applicable)),
            calculations: Some(vec![PointMeasurementCalculation {
                from_quantity: from.to_string(),
                method: "rmssd".into(),
                calculation_version: "provider-parser-v1".into(),
                window_s: Some(rmssd.window_s),
                usable_sample_count: Some(rmssd.n_beats as u64),
                dropped_sample_count: (rmssd.n_dropped > 0).then_some(rmssd.n_dropped as u64),
                signal_quality: quality,
                input_records: Vec::new(),
            }]),
        };
        Candidate::new(
            RecordData::PointMeasurement(data),
            MatchKey::measured(RecordType::PointMeasurement, "hrv_rmssd", &recorded_at),
            Section::Record,
        )
    }

    // ---- Finish ---------------------------------------------------------------

    fn finish(mut self) -> Result<()> {
        if self.short_windows > 0 {
            self.sink.skip_count("HRV beat windows too short or sparse for RMSSD", self.short_windows);
        }
        self.beat_readings()?;
        self.devices()?;
        self.samples()?;
        if let Some((_, error)) = self.deferred.time.take() {
            return Err(error);
        }
        if let Some((_, error)) = self.deferred.invalid.take() {
            return Err(error);
        }
        Ok(())
    }

    fn beat_readings(&mut self) -> Result<()> {
        self.scratch.settle()?;
        for seq in 0..self.beat_readings {
            let body: String =
                self.scratch
                    .connection()
                    .query_row("SELECT body FROM derived WHERE seq = ?1", [seq], |row| row.get(0))?;
            let reading: BeatReading = serde_json::from_str(&body).map_err(|error| Error::other(error.to_string()))?;
            let Some(recorded_at) = self.instant(Stage::BeatReadings, &reading.recorded_at) else {
                continue;
            };
            let receipt = xml::timestamp_receipt(&reading.input_raw, reading.zone.as_deref());
            let rmssd = hrv::Rmssd {
                rmssd_ms: reading.rmssd_ms,
                n_beats: reading.n_beats,
                n_dropped: reading.n_dropped,
                window_s: reading.window_s,
            };
            let candidate = self.rmssd_reading(&reading.source_id, recorded_at, receipt, "hrv_beats", rmssd, None);
            self.add(Stage::BeatReadings, candidate)?;
        }
        Ok(())
    }

    fn devices(&mut self) -> Result<()> {
        let devices: Vec<(writers::DeviceSeen, u64)> =
            self.writers.devices_seen().map(|(seen, count)| (seen.clone(), count)).collect();
        for (seen, count) in devices {
            let Device { name, manufacturer, model, hardware } = seen.device;
            let key = MatchKey::unique(vec![
                KeyPart::text("source_device"),
                KeyPart::Source,
                KeyPart::text(name.clone().unwrap_or_default()),
                KeyPart::text(manufacturer.clone().unwrap_or_default()),
                KeyPart::text(model.clone().unwrap_or_default()),
                KeyPart::text(hardware.clone().unwrap_or_default()),
            ]);
            let data = SourceDeviceData {
                source_id: seen.source_id,
                name,
                manufacturer,
                model,
                hardware,
                serial_number: None,
                coverage_start_date: Some(seen.from),
                coverage_end_date: Some(seen.to),
                accepted_record_count: Some(count),
            };
            self.add(Stage::Points, Candidate::new(RecordData::SourceDevice(data), key, Section::Device))?;
        }
        Ok(())
    }

    /// Send every sample, grouped and ordered as 2.1.0 listed them.
    fn samples(&mut self) -> Result<()> {
        self.scratch.settle()?;
        let mut after: Option<(i64, i64, i64, i64, i64)> = None;
        let mut group_start: Option<((i64, i64, i64), String, i64)> = None;
        loop {
            let rows = self.sample_page(after)?;
            let Some(last) = rows.last() else {
                break;
            };
            after = Some((last.phase, last.bucket, last.day, last.instant_ms, last.seq));
            for row in rows {
                let group = (row.phase, row.bucket, row.day);
                if group_start.as_ref().is_none_or(|(known, _, _)| *known != group) {
                    group_start = Some((group, row.at.clone(), row.instant_ms));
                }
                let (_, start_text, start_ms) = group_start.as_ref().expect("a group start");
                let candidate = self.sample_candidate(&row, start_text, *start_ms, self.spans.contains(&group))?;
                if let Some(candidate) = candidate {
                    self.add(Stage::Samples, candidate)?;
                }
            }
        }
        Ok(())
    }

    fn sample_page(&self, after: Option<(i64, i64, i64, i64, i64)>) -> Result<Vec<SampleRow>> {
        let after = after.unwrap_or((-1, -1, -1, i64::MIN, -1));
        let mut statement = self.scratch.connection().prepare_cached(
            "SELECT phase, bucket, day, instant_ms, seq, source_id, manual, quantity, at, value, duration_ms,
                    input_raw, input_value, input_unit, zone
               FROM samples
              WHERE (phase, bucket, day, instant_ms, seq) > (?1, ?2, ?3, ?4, ?5)
              ORDER BY phase, bucket, day, instant_ms, seq
              LIMIT ?6",
        )?;
        let rows = statement
            .query_map(
                params![after.0, after.1, after.2, after.3, after.4, crate::input_policy::IMPORT_BATCH_RECORDS as i64],
                |row| {
                    Ok(SampleRow {
                        phase: row.get(0)?,
                        bucket: row.get(1)?,
                        day: row.get(2)?,
                        instant_ms: row.get(3)?,
                        seq: row.get(4)?,
                        source_id: row.get(5)?,
                        manual: row.get(6)?,
                        quantity: row.get(7)?,
                        at: row.get(8)?,
                        value: row.get(9)?,
                        duration_ms: row.get(10)?,
                        input_raw: row.get(11)?,
                        input_value: row.get(12)?,
                        input_unit: row.get(13)?,
                        zone: row.get(14)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn sample_candidate(
        &mut self,
        row: &SampleRow,
        start_text: &str,
        start_ms: i64,
        spans: bool,
    ) -> Result<Option<Candidate>> {
        let quantity = SeriesQuantity::parse(&row.quantity)
            .ok_or_else(|| Error::other(format!("internal: unknown series quantity {}", row.quantity)))?;
        let offset = row.instant_ms - start_ms;
        let written = series_at_offset_of(start_text, start_ms + offset)?;
        let Some(recorded_at) = self.instant(Stage::Samples, &written) else {
            return Ok(None);
        };
        let (input_timestamp, base) = match &row.input_raw {
            Some(raw) => (xml::timestamp_receipt(raw, row.zone.as_deref()), None),
            None => {
                let offset = offset_text(&written);
                (
                    InputTimestampReceipt {
                        timestamp_text: Some(written.clone()),
                        utc_offset_meaning: offset.as_ref().map(|_| UtcOffsetMeaning::MeaningNotDocumented),
                        utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
                        utc_offset_text: offset,
                        source_time_zone: None,
                        source_time_zone_meaning: None,
                        timestamp_basis: Some(InputTimestampBasis::ReconstructedFromSeriesStart),
                        timestamp_missing_reason: None,
                    },
                    Some(start_text.to_string()),
                )
            }
        };
        let input = row.input_value.map(|input_value| SourceInput {
            input_value: Some(input_value),
            input_unit: row.input_unit.clone(),
            ..SourceInput::default()
        });
        let data = MeasurementSampleData {
            source_id: row.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::MeasurementSample, Some(quantity.as_str()), false, row.manual)),
            quantity,
            recorded_at: recorded_at.clone(),
            input_timestamp,
            value: source_value(row.value, quantity.unit(), input.as_ref()),
            duration_ms: spans.then_some(row.duration_ms.max(0) as u64),
            timestamp_reconstruction_method: base.as_ref().map(|_| "series_start_plus_offset_ms_v1".to_string()),
            timestamp_base_text: base,
        };
        Ok(Some(Candidate::new(
            RecordData::MeasurementSample(data),
            MatchKey::measured(RecordType::MeasurementSample, quantity.as_str(), &recorded_at),
            Section::Sample,
        )))
    }
}

struct SampleRow {
    phase: i64,
    bucket: i64,
    day: i64,
    instant_ms: i64,
    seq: i64,
    source_id: String,
    manual: bool,
    quantity: String,
    at: String,
    value: f64,
    duration_ms: i64,
    input_raw: Option<String>,
    input_value: Option<f64>,
    input_unit: Option<String>,
    zone: Option<String>,
}

/// A workout as it waits for its route files.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredWorkout {
    record: WorkoutRecord,
    source_id: String,
    manual: bool,
}

const SLEEP_GAP_MS: i64 = 3 * 60 * 60 * 1000;
const SLEEP_ASLEEP: [&str; 5] = [
    "HKCategoryValueSleepAnalysisAsleep",
    "HKCategoryValueSleepAnalysisAsleepCore",
    "HKCategoryValueSleepAnalysisAsleepDeep",
    "HKCategoryValueSleepAnalysisAsleepREM",
    "HKCategoryValueSleepAnalysisAsleepUnspecified",
];

/// One night as its fragments arrive in start order. Totals add up as they
/// arrive, so a night never holds its fragments.
struct Night {
    start: SleepFragment,
    end: SleepFragment,
    cluster_end: i64,
    device_raw: String,
    in_bed: f64,
    asleep: f64,
    deep: f64,
    rem: f64,
    light: f64,
    awake: f64,
    interruptions: u64,
}

impl Night {
    fn start(fragment: SleepFragment) -> Night {
        let mut night = Night {
            cluster_end: fragment.end_ms,
            start: fragment.clone(),
            end: fragment.clone(),
            device_raw: String::new(),
            in_bed: 0.0,
            asleep: 0.0,
            deep: 0.0,
            rem: 0.0,
            light: 0.0,
            awake: 0.0,
            interruptions: 0,
        };
        night.count(&fragment);
        night
    }

    fn extend(&mut self, fragment: SleepFragment) {
        self.cluster_end = self.cluster_end.max(fragment.end_ms);
        self.count(&fragment);
        if fragment.end_ms > self.end.end_ms {
            self.end = fragment;
        }
    }

    fn count(&mut self, fragment: &SleepFragment) {
        if self.device_raw.is_empty() && parse_device(&fragment.device_raw, Spaces::Ascii).is_some() {
            self.device_raw = fragment.device_raw.clone();
        }
        let seconds = (fragment.end_ms - fragment.start_ms) as f64 / 1000.0;
        match fragment.value.as_str() {
            "HKCategoryValueSleepAnalysisInBed" => self.in_bed += seconds,
            "HKCategoryValueSleepAnalysisAwake" => {
                self.awake += seconds;
                self.interruptions += 1;
            }
            value if SLEEP_ASLEEP.contains(&value) => {
                self.asleep += seconds;
                match value {
                    "HKCategoryValueSleepAnalysisAsleepDeep" => self.deep += seconds,
                    "HKCategoryValueSleepAnalysisAsleepREM" => self.rem += seconds,
                    "HKCategoryValueSleepAnalysisAsleepCore" => self.light += seconds,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// The night's totals, or nothing when it ends before it starts.
    fn aggregates(&self) -> Option<Vec<Aggregate>> {
        if self.end.end_ms <= self.start.start_ms {
            return None;
        }
        let mut out = Vec::new();
        let mut put = |name: &str, value: f64| out.push(Aggregate { name: name.into(), value, input: None });
        for (name, seconds) in [
            ("duration_s", self.asleep),
            ("time_in_bed_s", self.in_bed),
            ("deep_s", self.deep),
            ("rem_s", self.rem),
            ("light_s", self.light),
            ("awake_s", self.awake),
        ] {
            if seconds > 0.0 {
                put(name, hrv::helper_round(seconds));
            }
        }
        if self.interruptions > 0 {
            put("interruptions", self.interruptions as f64);
        }
        if self.in_bed > 0.0 && self.asleep > 0.0 {
            put("efficiency_pct", (hrv::helper_round((self.asleep / self.in_bed) * 1000.0) / 10.0).min(100.0));
        }
        Some(out)
    }
}

impl xml::Events for Import<'_, '_> {
    fn sleep_opened(&mut self, writer: &str, device_raw: &str, on_day: &str) -> Result<()> {
        self.writers.resolve(self.sink, writer, device_raw, on_day);
        Ok(())
    }

    fn sleep_fragment(&mut self, fragment: SleepFragment) -> Result<()> {
        let key =
            if writers::hand_entered(&fragment.writer) { "\u{0}manual".to_string() } else { fragment.writer.clone() };
        let next = self.fragment_groups.len() as i64;
        let group = *self.fragment_groups.entry(key).or_insert(next);
        let seq = self.fragment_seq;
        self.fragment_seq += 1;
        let body = serde_json::to_string(&fragment).map_err(|error| Error::other(error.to_string()))?;
        self.scratch
            .connection()
            .prepare_cached("INSERT INTO fragments (grp, start_ms, seq, body) VALUES (?1, ?2, ?3, ?4)")?
            .execute(params![group, fragment.start_ms, seq, body])?;
        Ok(())
    }

    fn point(&mut self, point: PointRecord) -> Result<()> {
        let writer = self.writers.resolve(self.sink, &point.writer, &point.device_raw, &point.on_day);
        if let Some(key) = &writer.device_key {
            self.writers.accept(key);
        }
        let Some(recorded_at) = self.instant(Stage::Points, &point.recorded_at) else {
            return Ok(());
        };
        let quantity = point.quantity;
        let input = SourceInput {
            input_value: Some(point.input_value),
            input_unit_missing_reason: point.input_unit.is_none().then_some(MissingReason::NotProvidedBySource),
            input_unit: point.input_unit,
            ..SourceInput::default()
        };
        let data = PointMeasurementData {
            source_id: writer.source_id.clone(),
            provider_record_id: None,
            provider_reporting_periods: None,
            received_at: self.sink.received_at().to_string(),
            received_at_basis: ReceivedAtBasis::ImportTime,
            supplied_fields: None,
            field_basis: Some(self.basis(RecordType::PointMeasurement, Some(quantity.as_str()), false, writer.manual)),
            quantity,
            recorded_at: Some(recorded_at.clone()),
            input_timestamp: xml::timestamp_receipt(&point.input_raw, point.source_time_zone.as_deref()),
            value: source_value(point.value, quantity.unit(), Some(&input)),
            calculations: None,
        };
        self.add(
            Stage::Points,
            Candidate::new(
                RecordData::PointMeasurement(data),
                MatchKey::measured(RecordType::PointMeasurement, quantity.as_str(), &recorded_at),
                Section::Record,
            ),
        )
    }

    fn sample(&mut self, sample: SeriesRecord) -> Result<()> {
        let writer = self.writers.resolve(self.sink, &sample.writer, &sample.device_raw, &sample.on_day);
        let next = self.series_buckets.len() as i64;
        let bucket = *self.series_buckets.entry((sample.quantity, writer.source_id.clone())).or_insert(next);
        let day: String = sample.at.chars().take(10).collect();
        let day_index = self.day_index(PHASE_SERIES, bucket, &day);
        self.stage_sample(
            (PHASE_SERIES, bucket, day_index),
            &writer.source_id,
            writer.manual,
            sample.quantity,
            &sample.at,
            sample.value,
            sample.duration_ms,
            Some((
                &sample.input_raw,
                sample.input_value,
                sample.input_unit.as_deref(),
                sample.source_time_zone.as_deref(),
            )),
        )?;
        let (beats, rmssd) = sample.beat_samples();
        if beats.is_empty() {
            return Ok(());
        }
        let next = self.beat_buckets.len() as i64;
        let beat_bucket = *self.beat_buckets.entry((writer.source_id.clone(), day)).or_insert(next);
        for beat in &beats {
            self.stage_sample(
                (PHASE_BEATS, beat_bucket, 0),
                &writer.source_id,
                writer.manual,
                SeriesQuantity::HrvBeats,
                &beat.at,
                beat.value,
                0,
                None,
            )?;
        }
        match rmssd {
            Some(rmssd) => {
                let reading = BeatReading {
                    source_id: writer.source_id.clone(),
                    recorded_at: sample.at.clone(),
                    input_raw: sample.input_raw.clone(),
                    zone: sample.source_time_zone.clone(),
                    rmssd_ms: rmssd.rmssd_ms,
                    window_s: rmssd.window_s,
                    n_beats: rmssd.n_beats,
                    n_dropped: rmssd.n_dropped,
                };
                let body = serde_json::to_string(&reading).map_err(|error| Error::other(error.to_string()))?;
                self.scratch
                    .connection()
                    .prepare_cached("INSERT INTO derived (seq, body) VALUES (?1, ?2)")?
                    .execute(params![self.beat_readings, body])?;
                self.beat_readings += 1;
            }
            None => self.short_windows += 1,
        }
        Ok(())
    }

    fn workout(&mut self, workout: WorkoutRecord) -> Result<()> {
        let writer = self.writers.resolve(self.sink, &workout.writer, &workout.device_raw, &workout.on_day);
        if let Some(key) = &writer.device_key {
            self.writers.accept(key);
        }
        let seq = self.workouts.len() as i64;
        self.workouts.push(WorkoutMeta {
            seq,
            start_ms: parse_instant(&workout.start),
            end_ms: parse_instant(&workout.end),
            has_inline_route: !workout.route.is_empty(),
            has_laps: !workout.laps.is_empty(),
            file_references: workout.file_references.clone(),
            elevation_gain_m: None,
            splits_from: None,
            route_summaries_calculated: false,
        });
        let stored = StoredWorkout { record: workout, source_id: writer.source_id, manual: writer.manual };
        let body = serde_json::to_string(&stored).map_err(|error| Error::other(error.to_string()))?;
        self.scratch.connection().execute("INSERT INTO workouts (seq, body) VALUES (?1, ?2)", params![seq, body])?;
        Ok(())
    }

    fn activity(&mut self, activity: ActivityRecord) -> Result<()> {
        let writer = self.writers.resolve(self.sink, &activity.writer, &activity.device_raw, &activity.on_day);
        let period = ProviderReportingPeriod {
            period_type: ProviderReportingPeriodType::AppleActivitySummaryDay,
            provider_period_id: None,
            provider_date: Some(activity.date.clone()),
            start_at: None,
            end_at: None,
            input_start_timestamp: None,
            input_end_timestamp: None,
            timezone_offset_text: None,
            timezone_offset_meaning: None,
            timezone_offset_missing_reason: Some(MissingReason::NotProvidedBySource),
        };
        for value in activity.values {
            let input = SourceInput {
                input_value: Some(value.input_value),
                input_unit: value.input_unit,
                ..SourceInput::default()
            };
            let data = ProviderSummaryData {
                source_id: writer.source_id.clone(),
                provider_record_id: None,
                provider_reporting_periods: Some(vec![period.clone()]),
                received_at: self.sink.received_at().to_string(),
                received_at_basis: ReceivedAtBasis::ImportTime,
                supplied_fields: None,
                field_basis: Some(self.basis(RecordType::ProviderSummary, None, false, writer.manual)),
                summary_type: value.summary_type.to_string(),
                value: source_value(value.value, value.standard_unit, Some(&input)),
                recorded_at: None,
                input_timestamp: missing_timestamp_receipt(),
            };
            self.add(
                Stage::Summaries,
                Candidate::new(
                    RecordData::ProviderSummary(data),
                    MatchKey::measured(RecordType::ProviderSummary, value.summary_type, &activity.date),
                    Section::Summary,
                ),
            )?;
        }
        Ok(())
    }

    fn skip(&mut self, reason: &str, example: Option<String>, count: u64) {
        match example {
            Some(example) => {
                for _ in 0..count {
                    self.sink.skip(reason, &example);
                }
            }
            None => self.sink.skip_count(reason, count),
        }
    }
}

fn problem_detail(problem: &EcgProblem) -> &str {
    match problem {
        EcgProblem::Unreadable(detail) | EcgProblem::NotSinus(detail) | EcgProblem::TooFewBeats(detail) => detail,
    }
}

/// `Math.round`.
fn js_round(value: f64) -> f64 {
    crate::model::grade::js_round(value)
}

/// `new Date(ms).toISOString()`, or the `RangeError` JavaScript throws.
fn iso_string(millis: i64) -> Result<String> {
    const LIMIT: i64 = 8_640_000_000_000_000;
    if !(-LIMIT..=LIMIT).contains(&millis) {
        return Err(Error::other("Invalid time value"));
    }
    let days = millis.div_euclid(86_400_000);
    let in_day = millis.rem_euclid(86_400_000);
    let (year, month, day) = crate::model::time::civil_from_days(days);
    let year_text = if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year < 0 {
        format!("-{:06}", -year)
    } else {
        format!("+{year:06}")
    };
    Ok(format!(
        "{year_text}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1000 % 60,
        in_day % 1000
    ))
}

/// The trailing `±HH:MM` of a reference time as a sign and milliseconds.
fn colon_offset(reference: &str) -> Option<(&str, i64)> {
    let bytes = reference.as_bytes();
    if bytes.len() < 6 {
        return None;
    }
    let tail = &bytes[bytes.len() - 6..];
    let digits = |slice: &[u8]| slice.iter().all(u8::is_ascii_digit);
    if !(matches!(tail[0], b'+' | b'-') && digits(&tail[1..3]) && tail[3] == b':' && digits(&tail[4..6])) {
        return None;
    }
    let text = &reference[reference.len() - 6..];
    let hours: i64 = text[1..3].parse().ok()?;
    let minutes: i64 = text[4..6].parse().ok()?;
    let sign = if tail[0] == b'-' { -1 } else { 1 };
    Some((text, sign * (hours * 60 + minutes) * 60_000))
}

/// `atOffsetOf` from `src/import/apple.ts`, for ECG beats: always with
/// milliseconds.
fn ecg_at_offset_of(reference: &str, instant_ms: f64) -> Result<String> {
    if !instant_ms.is_finite() {
        return Err(Error::other("Invalid time value"));
    }
    let rounded = js_round(instant_ms) as i64;
    match colon_offset(reference) {
        None => iso_string(rounded),
        Some((offset, shift)) => {
            let shifted = iso_string(rounded + shift)?;
            Ok(format!("{}{offset}", &shifted[..23]))
        }
    }
}

/// `atOffsetOf` from `src/series.ts`: milliseconds only when there are some.
fn series_at_offset_of(reference: &str, instant_ms: i64) -> Result<String> {
    let iso = iso_string(instant_ms)?;
    let whole_second = instant_ms % 1000 == 0;
    match colon_offset(reference) {
        None => Ok(if whole_second { format!("{}Z", &iso[..19]) } else { iso }),
        Some((offset, shift)) => {
            let shifted = iso_string(instant_ms + shift)?;
            let clock = if whole_second { &shifted[..19] } else { &shifted[..23] };
            Ok(format!("{clock}{offset}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_strings_match_javascript() {
        assert_eq!(iso_string(0).unwrap(), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_string(-1).unwrap(), "1969-12-31T23:59:59.999Z");
        assert_eq!(iso_string(253_402_300_800_000).unwrap(), "+010000-01-01T00:00:00.000Z");
        assert!(iso_string(8_640_000_000_000_001).is_err());
    }

    #[test]
    fn sample_times_keep_their_offset_and_drop_empty_milliseconds() {
        let start = "2026-08-09T06:12:00-07:00";
        let start_ms = parse_instant(start).unwrap();
        assert_eq!(series_at_offset_of(start, start_ms).unwrap(), "2026-08-09T06:12:00-07:00");
        assert_eq!(series_at_offset_of(start, start_ms + 220).unwrap(), "2026-08-09T06:12:00.220-07:00");
        assert_eq!(series_at_offset_of("2026-08-09T06:12:00Z", start_ms).unwrap(), "2026-08-09T13:12:00Z");
        assert_eq!(ecg_at_offset_of(start, start_ms as f64 + 0.4).unwrap(), "2026-08-09T06:12:00.000-07:00");
    }
}
