//! The provider import operation: read, stage, match, preview, and write.
//!
//! A provider reader sends typed candidates to a `CandidateSink`. The sink
//! validates each one and stages it in a private temporary database. Matching
//! then reads the live database inside one read transaction: it resolves
//! sources, finds each candidate's stored record by its match key, and decides
//! whether the candidate is new, matching, changed, or a conflict. The preview
//! is built from that staged plan and the database revision it was read at.
//!
//! `ProviderPreview::apply` writes the plan in one write transaction. It first
//! checks that the database revision has not moved, and each correction checks
//! its record's revision. A stale plan is refused. No write lock is held while
//! an export is read or while a person reviews the preview.
//!
//! A route declares how its candidates are matched (`MatchPolicy`):
//!
//! - `Provider`: Apple Health, WHOOP, and Oura. These are the rules of
//!   `src/db/provider-import.ts` in 2.1.0: numbered keys, collision groups, and
//!   a conflict for a key that names a deleted record.
//! - `Keyed`: Garmin and Strava files, the Garmin pull, and mapped imports.
//!   These are the rules of the 2.1.0 helper's store (`insert_record` and
//!   `insert_group`): one plain key per record, no collision groups, and a
//!   deleted record or source left out rather than written again (D182, D190).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, OptionalExtension};
use serde::ser::{SerializeMap, SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};

use crate::error::{Error, Result};
use crate::input_policy::IMPORT_BATCH_RECORDS;
use crate::json::{self, Value};
use crate::model::*;
use crate::store::write::{self, StaleStyle};
use crate::store::{self, read, Changes, Correction, ExtensionsUpdate, NewRecord, Store};

use super::candidate::{same_logical_data, Candidate, KeyPart, MatchKey};
use super::collate::locale_compare;
use super::detect::{self, DetectedExport, ExportFormat};
use super::staging::{Action, Decision, Group, PlannedRecord, Staging};

/// What identifies a provider route in previews, sources, and match keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The preview's `provider`, such as `whoop`.
    pub provider: String,
    /// The preview's `imported`, such as `WHOOP export`.
    pub label: String,
    /// The vendor name in source details, such as `WHOOP`.
    pub vendor_label: String,
    /// The source `data_route`, such as `whoop_csv`.
    pub data_route: String,
    /// How the export was laid out, such as `export folder`.
    pub container: String,
    /// The match key method, such as `whoop_csv/v1`.
    pub match_key_method: String,
    /// How candidates find their stored records.
    pub policy: MatchPolicy,
    /// The result the 2.1.0 command printed for this route.
    pub report: ReportKind,
}

impl Route {
    /// The route of a detected Apple Health, WHOOP, or Oura export.
    pub fn for_export(export: &DetectedExport) -> Route {
        let format = export.format;
        Route {
            provider: format.as_str().to_string(),
            label: format!("{} export", format.vendor_label()),
            vendor_label: format.vendor_label().to_string(),
            data_route: format.data_route().to_string(),
            container: export.container_label().to_string(),
            match_key_method: format!("{}/v1", format.data_route()),
            policy: MatchPolicy::Provider,
            report: ReportKind::Provider,
        }
    }
}

/// How a route's candidates find their stored records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPolicy {
    /// Apple Health, WHOOP, and Oura (2.1.0 `src/db/provider-import.ts`).
    ///
    /// Keys are numbered and form collision groups. A key that names a deleted
    /// record or another record type is a conflict. A source is found by
    /// writer, route, and sensor among sources that are not deleted, and it
    /// has an import key of its own.
    Provider,
    /// Garmin and Strava files, the Garmin pull, and mapped imports (the 2.1.0
    /// helper's store).
    ///
    /// Each record has one plain key. A key that names a deleted record leaves
    /// the candidate out with a line that says so, as does a deleted source
    /// (D182, D190). A key that names another record type refuses the import.
    /// A source is found by writer, route, and sensor whether or not it is
    /// deleted, and it has no import key. A later candidate with an earlier
    /// candidate's key repeats it when the data agree and refuses the import
    /// when they differ.
    Keyed(KeyedPolicy),
}

/// What a keyed route does with a stored record whose data changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyedPolicy {
    /// The new data is laid over the stored data, so a field the source left
    /// out keeps its stored value. The 2.1.0 Garmin, Strava, and pull routes
    /// merged this way (`revise`); mapped imports replace the whole object (D182).
    pub merge: bool,
    /// A changed record keeps its first `received_at` (D182).
    pub keep_received_at: bool,
    /// The basis of a correction's revision.
    pub correction_basis: BecameKnownAtBasis,
}

/// The result a route reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportKind {
    /// `ath import <export> --json` for Apple Health, WHOOP, and Oura.
    Provider,
    /// Garmin and Strava files: `source`, `record_ids`, `understood`,
    /// `preserved`, and `left_out`.
    Shared { source: String },
    /// The Garmin pull: `source`, `record_ids`, `understood`, and `preserved`.
    Pull { source: String },
    /// A mapped import: `mapping`, `record_ids`, `outcome`, `understood`,
    /// `preserved`, and `left_out`.
    Mapped { mapping: String },
}

/// A provider route's reader. It turns an export into candidates.
///
/// A reader depends on the record model and the input helpers in this module.
/// It never opens the athlete database, never decides whether a record is
/// new, and never prompts.
pub trait ProviderReader {
    /// The route for this export. Most readers keep the default.
    fn route(&self, export: &DetectedExport) -> Route {
        Route::for_export(export)
    }

    /// Read the export. Send every source, record, and omitted object to `sink`.
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()>;
}

/// A reader that holds its own input: a Garmin or Strava path, a Garmin
/// pull's responses, or a mapped JSON file with its mapping. It follows the
/// same rules as a `ProviderReader`.
pub trait RouteReader {
    fn route(&self) -> Route;

    /// Read the input. Send every source, record, and omitted object to `sink`.
    fn read(&self, sink: &mut CandidateSink<'_>) -> Result<()>;
}

/// The reader for a detected format, when this build has one.
pub fn reader_for(format: ExportFormat) -> Option<&'static dyn ProviderReader> {
    match format {
        ExportFormat::Whoop => Some(&super::whoop::WhoopReader),
        ExportFormat::Oura => Some(&super::oura::OuraReader),
        ExportFormat::Apple => Some(&super::apple::AppleReader),
    }
}

/// Options for a provider import preview.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// The import time stored as `received_at`, `created_at`, revision
    /// knowledge time, and import-key time. Defaults to now.
    pub received_at: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceEntry {
    provisional_id: String,
    data: SourceData,
}

/// One skip reason with its count and first example.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// Skip reasons with their counts and first examples, in preview order.
pub type SkipList = Vec<(String, Skipped)>;

/// What a keyed route reports beside its records: the input fields it
/// understood, the fields it kept under their own names, and what it left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteReport {
    /// Sorted by bytes, each once.
    pub understood: std::collections::BTreeSet<String>,
    /// Sorted by bytes, each once.
    pub preserved: std::collections::BTreeSet<String>,
    /// In input order. Lines that matching adds follow their group's place.
    left_out: Vec<(i64, String)>,
    /// Fields read for records under a source, by field. A field counts only
    /// when one of its sources is not deleted.
    understood_under: std::collections::BTreeMap<String, Vec<String>>,
    preserved_under: std::collections::BTreeMap<String, Vec<String>>,
}

impl RouteReport {
    /// The left-out lines in input order.
    pub fn left_out(&self) -> Vec<String> {
        let mut lines = self.left_out.clone();
        lines.sort_by_key(|(seq, _)| *seq);
        lines.into_iter().map(|(_, line)| line).collect()
    }
}

/// Where a reader sends what it found.
pub struct CandidateSink<'a> {
    route: &'a Route,
    received_at: &'a str,
    staging: &'a mut Staging,
    sources: Vec<SourceEntry>,
    skipped: SkipList,
    pending: usize,
    report: RouteReport,
    group: Option<Group>,
    next_seq: i64,
}

impl<'a> CandidateSink<'a> {
    fn new(route: &'a Route, received_at: &'a str, staging: &'a mut Staging) -> Result<Self> {
        staging.begin()?;
        Ok(CandidateSink {
            route,
            received_at,
            staging,
            sources: Vec::new(),
            skipped: Vec::new(),
            pending: 0,
            report: RouteReport::default(),
            group: None,
            next_seq: 0,
        })
    }

    /// The import time to store as each measured record's `received_at`.
    pub fn received_at(&self) -> &str {
        self.received_at
    }

    pub fn route(&self) -> &Route {
        self.route
    }

    /// The source for records a named writer exported through this route.
    /// Returns the source ID to put in each record's `source_id`.
    ///
    /// One writer, route, and sensor is one source. A second call with the
    /// same writer and sensor returns the same ID and takes the newer detail.
    /// The detail reads `WHOOP export folder` when the writer is the vendor,
    /// otherwise `<writer> [sensor] via <vendor> <container>`.
    pub fn export_source(&mut self, writer: &str, vendor: &str, sensor: Option<&str>) -> String {
        let route = self.route;
        let who = match sensor {
            Some(sensor) if !sensor.is_empty() => format!("{writer} {sensor}"),
            _ => writer.to_string(),
        };
        let detail = if writer == route.vendor_label {
            format!("{} {}", route.vendor_label, route.container)
        } else {
            format!("{who} via {} {}", route.vendor_label, route.container)
        };
        let sensor = sensor.filter(|sensor| !sensor.is_empty()).map(str::to_string);
        if let Some(existing) = self.sources.iter_mut().find(|entry| {
            entry.data.source_type == SourceType::ExportFile
                && entry.data.data_writer == writer
                && entry.data.sensor_type == sensor
        }) {
            existing.data.detail = Some(detail);
            return existing.provisional_id.clone();
        }
        let provisional_id = crate::ids::new_record_id();
        self.sources.push(SourceEntry {
            provisional_id: provisional_id.clone(),
            data: SourceData {
                source_type: SourceType::ExportFile,
                data_writer: writer.to_string(),
                data_route: route.data_route.clone(),
                vendor: Some(vendor.to_string()),
                sensor_type: sensor,
                detail: Some(detail),
                supplied_fields: None,
            },
        });
        provisional_id
    }

    /// The source for a keyed route's records from one writer and sensor.
    /// Returns the source ID to put in each record's `source_id`.
    ///
    /// The source names the writer, this route, the vendor, and the sensor when
    /// there is one. It has no detail, as the 2.1.0 helper wrote none. A second
    /// call with the same writer and sensor returns the same ID.
    pub fn route_source(&mut self, writer: &str, vendor: &str, sensor: Option<&str>) -> String {
        let sensor = sensor.filter(|sensor| !sensor.is_empty()).map(str::to_string);
        if let Some(existing) = self.sources.iter().find(|entry| {
            entry.data.source_type == SourceType::ExportFile
                && entry.data.data_writer == writer
                && entry.data.sensor_type == sensor
        }) {
            return existing.provisional_id.clone();
        }
        let provisional_id = crate::ids::new_record_id();
        self.sources.push(SourceEntry {
            provisional_id: provisional_id.clone(),
            data: SourceData {
                source_type: SourceType::ExportFile,
                data_writer: writer.to_string(),
                data_route: self.route.data_route.clone(),
                vendor: Some(vendor.to_string()),
                sensor_type: sensor,
                detail: None,
                supplied_fields: None,
            },
        });
        provisional_id
    }

    /// Start a keyed route's group: the records of one source object, file, or
    /// row. Candidates added after this call belong to it. When matching leaves
    /// one out, the line starts with `label` and takes the group's place among
    /// the other left-out lines.
    pub fn begin_group(&mut self, label: impl Into<String>) {
        self.group = Some(Group { seq: self.next_seq, label: label.into() });
        self.next_seq += 1;
    }

    /// Report an omitted object of a keyed route, in input order.
    pub fn leave_out(&mut self, line: impl Into<String>) {
        self.report.left_out.push((self.next_seq, line.into()));
        self.next_seq += 1;
    }

    /// Note an input field the route stored as a typed value.
    pub fn understood(&mut self, field: impl Into<String>) {
        self.report.understood.insert(field.into());
    }

    /// Note an input field the route kept under its own name.
    pub fn preserved(&mut self, field: impl Into<String>) {
        self.report.preserved.insert(field.into());
    }

    /// Note a field read for a record under `source_id`. It is reported only
    /// when that source is not deleted, as 2.1.0 read no field of a row whose
    /// source was deleted (D190).
    pub fn understood_under(&mut self, source_id: &str, field: impl Into<String>) {
        note_under(&mut self.report.understood_under, source_id, field.into());
    }

    /// Note a field kept under its own name in a record under `source_id`.
    pub fn preserved_under(&mut self, source_id: &str, field: impl Into<String>) {
        note_under(&mut self.report.preserved_under, source_id, field.into());
    }

    /// The source for values a person typed in. Every route shares one manual
    /// source, and an existing one is reused without a new revision.
    pub fn manual_source(&mut self) -> String {
        if let Some(existing) = self.sources.iter().find(|entry| entry.data.source_type == SourceType::Manual) {
            return existing.provisional_id.clone();
        }
        let provisional_id = crate::ids::new_record_id();
        self.sources.push(SourceEntry {
            provisional_id: provisional_id.clone(),
            data: SourceData {
                source_type: SourceType::Manual,
                data_writer: "ath".into(),
                data_route: "manual_entry".into(),
                vendor: None,
                sensor_type: None,
                detail: Some("Manually recorded data".into()),
                supplied_fields: None,
            },
        });
        provisional_id
    }

    /// True when a source ID from this sink is the manual source.
    pub fn is_manual_source(&self, source_id: &str) -> bool {
        self.sources
            .iter()
            .any(|entry| entry.provisional_id == source_id && entry.data.source_type == SourceType::Manual)
    }

    /// Validate and stage one candidate.
    pub fn add(&mut self, candidate: Candidate) -> Result<()> {
        if let Some(source_id) = candidate.data.source_id() {
            if !self.sources.iter().any(|entry| entry.provisional_id == source_id) {
                return Err(Error::other(format!(
                    "A {} candidate names a source the reader did not declare",
                    candidate.data.record_type()
                )));
            }
        }
        let record_id = crate::ids::new_record_id();
        let data = candidate.data.to_value();
        check_record(&record_id, self.received_at, candidate.data.record_type(), &data)?;
        self.staging.insert(
            candidate.section as i64,
            &record_id,
            &json::to_string(&data),
            &candidate,
            None,
            self.group.as_ref(),
        )?;
        self.pending += 1;
        if self.pending >= IMPORT_BATCH_RECORDS {
            self.staging.commit()?;
            self.staging.begin()?;
            self.pending = 0;
        }
        Ok(())
    }

    /// Count one omitted row or object under a reason, and keep the first example.
    pub fn skip(&mut self, reason: &str, example: &str) {
        self.count_skip(reason, 1, Some(example));
    }

    /// Count omitted objects under a reason without an example.
    pub fn skip_count(&mut self, reason: &str, count: u64) {
        self.count_skip(reason, count, None);
    }

    fn count_skip(&mut self, reason: &str, count: u64, example: Option<&str>) {
        match self.skipped.iter_mut().find(|(known, _)| known == reason) {
            Some((_, skipped)) => {
                skipped.count += count;
                if skipped.example.is_none() {
                    skipped.example = example.map(str::to_string);
                }
            }
            None => self.skipped.push((reason.to_string(), Skipped { count, example: example.map(str::to_string) })),
        }
    }

    fn finish(self) -> Result<(Vec<SourceEntry>, SkipList, RouteReport)> {
        self.staging.commit()?;
        let mut skipped = self.skipped;
        skipped.sort_by(|(left, _), (right, _)| locale_compare(left, right));
        Ok((self.sources, skipped, self.report))
    }
}

fn note_under(notes: &mut std::collections::BTreeMap<String, Vec<String>>, source_id: &str, field: String) {
    let sources = notes.entry(field).or_default();
    if !sources.iter().any(|known| known == source_id) {
        sources.push(source_id.to_string());
    }
}

/// Validate data as the complete current record it would become, so an
/// error names the same path as a stored record would: `data.value...`.
fn check_record(record_id: &str, created_at: &str, record_type: RecordType, data: &Value) -> Result<()> {
    let mut record = json::Map::new();
    record.insert("record_id".into(), Value::String(record_id.to_string()));
    record.insert("created_at".into(), Value::String(created_at.to_string()));
    record.insert("current_revision".into(), Value::from(1));
    record.insert("is_deleted".into(), Value::Bool(false));
    record.insert("record_type".into(), Value::String(record_type.as_str().into()));
    record.insert("data".into(), data.clone());
    CurrentRecord::from_value(&Value::Object(record))?;
    Ok(())
}

/// A record a preview lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedRecordRef {
    pub record_id: String,
    pub record_type: String,
    /// The revision a changed record has now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_revision: Option<u64>,
}

/// A record the import will not decide for the athlete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportConflict {
    pub match_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    pub message: String,
}

/// The preview of a provider import, built from a staged plan.
///
/// It serializes with the fields of `ath import <export> --json` in 2.1.0.
/// Record lists are read from the staged plan while serializing, so a large
/// plan is not held in memory.
pub struct ProviderPreview {
    pub provider: String,
    pub imported: String,
    pub destination_database_revision: u64,
    pub conflicts: Vec<ImportConflict>,
    /// Skip reasons in the 2.1.0 order, with counts and first examples.
    pub skipped: SkipList,
    pub route_points_stored: u64,
    database_id: String,
    received_at: String,
    match_key_method: String,
    counts: [u64; 3],
    staging: Staging,
    route: Route,
    report: RouteReport,
    /// A keyed route's sources that do not exist yet, with their new IDs.
    new_sources: Vec<(String, SourceData)>,
}

impl std::fmt::Debug for ProviderPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderPreview")
            .field("provider", &self.provider)
            .field("destination_database_revision", &self.destination_database_revision)
            .field("new", &self.counts[0])
            .field("matching", &self.counts[1])
            .field("changed", &self.counts[2])
            .field("conflicts", &self.conflicts)
            .finish()
    }
}

/// The result of a confirmed provider import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderImportResult {
    pub database_revision: u64,
    pub new_record_count: u64,
    pub changed_record_count: u64,
    pub matching_record_count: u64,
}

/// Detect a provider export and preview its import.
pub fn preview(store: &Store, path: &Path, options: &ImportOptions) -> Result<ProviderPreview> {
    let export = detect::detect_export(path)?;
    let reader = reader_for(export.format).ok_or_else(|| {
        Error::other(format!("{} import is not yet available in the native build", export.format.vendor_label()))
    })?;
    preview_export(store, reader, &export, options)
}

/// Preview an export with a given reader.
pub fn preview_export(
    store: &Store,
    reader: &dyn ProviderReader,
    export: &DetectedExport,
    options: &ImportOptions,
) -> Result<ProviderPreview> {
    preview_with(store, reader.route(export), options, |sink| reader.read(export, sink))
}

/// Preview the input a route reader holds.
pub fn preview_route(store: &Store, reader: &dyn RouteReader, options: &ImportOptions) -> Result<ProviderPreview> {
    preview_with(store, reader.route(), options, |sink| reader.read(sink))
}

fn preview_with(
    store: &Store,
    route: Route,
    options: &ImportOptions,
    read: impl FnOnce(&mut CandidateSink<'_>) -> Result<()>,
) -> Result<ProviderPreview> {
    let received_at = options.received_at.clone().unwrap_or_else(time::now_utc);
    if !time::is_utc_timestamp(&received_at) {
        return Err(Error::Invalid(ValidationError::single(
            vec![PathSegment::Key("received_at".into())],
            "Invalid ISO datetime",
        )));
    }
    let mut staging = Staging::create()?;
    let (sources, skipped, mut report) = {
        let mut sink = CandidateSink::new(&route, &received_at, &mut staging)?;
        read(&mut sink)?;
        sink.finish()?
    };
    let (destination_database_revision, new_sources) = match route.policy {
        MatchPolicy::Provider => (match_plan(store, &route, &received_at, &mut staging, &sources)?, Vec::new()),
        MatchPolicy::Keyed(policy) => {
            match_keyed(store, &route, policy, &received_at, &mut staging, &sources, &mut report)?
        }
    };
    let conflicts = staging
        .conflicts()?
        .into_iter()
        .map(|(match_key, record_id, message)| ImportConflict { match_key, record_id, message })
        .collect();
    Ok(ProviderPreview {
        provider: route.provider.clone(),
        imported: route.label.clone(),
        destination_database_revision,
        conflicts,
        skipped,
        route_points_stored: staging.route_points()?,
        database_id: store.metadata().database_id.clone(),
        received_at,
        match_key_method: route.match_key_method.clone(),
        counts: [staging.count(Action::New)?, staging.count(Action::Matching)?, staging.count(Action::Changed)?],
        staging,
        route,
        report,
        new_sources,
    })
}

const DUPLICATE_SAME: &str = "provider export repeats one import key";
const DUPLICATE_DIFFERENT: &str = "provider export gives one import key two different records";
const DELETED: &str = "provider import key points to a normally deleted record";
const AMBIGUOUS_GROUP: &str =
    "an indistinguishable duplicate group changed; the importer will not guess which row was corrected";

/// An existing source with the same writer, route, and sensor. The manual
/// source is the oldest manual source.
fn existing_source(connection: &rusqlite::Connection, data: &SourceData) -> Result<Option<String>> {
    if data.source_type == SourceType::Manual {
        return Ok(connection
            .query_row(
                "SELECT sources.record_id FROM sources
                   JOIN records ON records.record_id = sources.record_id
                  WHERE sources.source_type = 'manual' AND records.is_deleted = 0
                  ORDER BY records.created_at, sources.record_id
                  LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?);
    }
    Ok(connection
        .query_row(
            "SELECT sources.record_id FROM sources
               JOIN records ON records.record_id = sources.record_id
              WHERE sources.data_writer = ?1
                AND sources.data_route = ?2
                AND COALESCE(sources.sensor_type, '') = COALESCE(?3, '')
                AND records.is_deleted = 0
              LIMIT 1",
            params![data.data_writer, data.data_route, data.sensor_type],
            |row| row.get(0),
        )
        .optional()?)
}

fn record_by_import_key(connection: &rusqlite::Connection, method: &str, key: &str) -> Result<Option<CurrentRecord>> {
    let mut statement = connection.prepare_cached(
        "SELECT records.record_id, records.record_type, records.current_revision, records.created_at,
                records.is_deleted, record_revisions.record_json, record_revisions.extensions_json
           FROM record_import_keys
           JOIN records ON records.record_id = record_import_keys.record_id
           JOIN record_revisions
             ON record_revisions.record_id = records.record_id
            AND record_revisions.revision = records.current_revision
          WHERE record_import_keys.match_key_method = ?1 AND record_import_keys.match_key = ?2",
    )?;
    let mut rows = statement.query(params![method, key])?;
    match rows.next()? {
        Some(row) => Ok(Some(read::current_from_row(row)?)),
        None => Ok(None),
    }
}

/// Resolve sources and decide every staged candidate against the live
/// database. Returns the database revision the decisions were read at.
fn match_plan(
    store: &Store,
    route: &Route,
    received_at: &str,
    staging: &mut Staging,
    sources: &[SourceEntry],
) -> Result<u64> {
    let read_tx = store.connection().unchecked_transaction()?;
    let connection: &rusqlite::Connection = &read_tx;
    let revision = store::database_revision(connection)?;

    let mut resolved: HashMap<String, String> = HashMap::new();
    staging.begin()?;
    for source in sources {
        let existing = existing_source(connection, &source.data)?;
        let record_id = existing.clone().unwrap_or_else(|| source.provisional_id.clone());
        resolved.insert(source.provisional_id.clone(), record_id.clone());
        if source.data.source_type == SourceType::Manual && existing.is_some() {
            continue;
        }
        let candidate = Candidate::new(
            RecordData::Source(source.data.clone()),
            MatchKey::unique(vec![
                KeyPart::text("source"),
                KeyPart::text(&source.data.data_writer),
                KeyPart::text(&source.data.data_route),
                KeyPart::text(source.data.sensor_type.clone().unwrap_or_default()),
            ]),
            super::candidate::Section::Device,
        );
        let data = candidate.data.to_value();
        check_record(&record_id, received_at, RecordType::Source, &data)?;
        staging.insert(0, &record_id, &json::to_string(&data), &candidate, existing.as_deref(), None)?;
    }
    staging.commit()?;

    let mut after = None;
    loop {
        let rows = staging.rows_after(after, IMPORT_BATCH_RECORDS)?;
        let Some(last) = rows.last() else {
            break;
        };
        after = Some((last.section, last.position));
        staging.begin()?;
        for row in rows {
            let source_id = match &row.source_id {
                Some(provisional) => Some(
                    resolved
                        .get(provisional)
                        .cloned()
                        .ok_or_else(|| Error::other("A staged candidate names an unknown source"))?,
                ),
                None => None,
            };
            let mut data = json::parse(&row.data_json)?;
            if let (Some(final_id), Some(provisional)) = (&source_id, &row.source_id) {
                if final_id != provisional {
                    if let Value::Object(map) = &mut data {
                        map.insert("source_id".into(), Value::String(final_id.clone()));
                    }
                }
            }
            let data_json = json::to_string(&data);

            let Value::Array(mut parts) = json::parse(&row.key_parts)? else {
                return Err(Error::other("Staged key parts are not an array"));
            };
            for part in parts.iter_mut() {
                if part.is_null() {
                    let id = source_id.clone().ok_or_else(|| Error::other("A match key names a missing source"))?;
                    *part = Value::String(id);
                }
            }
            let collision_group = row.numbered.then(|| json::to_string(&Value::Array(parts.clone())));
            if let Some(ordinal) = row.ordinal {
                parts.push(Value::from(ordinal));
            }
            let match_key = json::to_string(&Value::Array(parts));

            if let Some((first, _, _)) = staging.first_with_key(&match_key, row.position)? {
                let message =
                    if same_logical_data(&json::parse(&first)?, &data) { DUPLICATE_SAME } else { DUPLICATE_DIFFERENT };
                staging.add_conflict(&match_key, None, message)?;
                staging.decide(
                    row.position,
                    &Decision {
                        record_id: &row.record_id,
                        data_json: &data_json,
                        match_key: &match_key,
                        collision_group: None,
                        action: Action::Conflict,
                        current_revision: None,
                    },
                )?;
                continue;
            }

            let mut current = record_by_import_key(connection, &route.match_key_method, &match_key)?;
            if current.is_none() {
                if let Some(existing) = &row.existing_record_id {
                    current = read::get_record(connection, existing)?;
                }
            }
            let Some(current) = current else {
                staging.decide(
                    row.position,
                    &Decision {
                        record_id: &row.record_id,
                        data_json: &data_json,
                        match_key: &match_key,
                        collision_group: collision_group.as_deref(),
                        action: Action::New,
                        current_revision: None,
                    },
                )?;
                continue;
            };
            if current.record_type().as_str() != row.record_type || current.is_deleted {
                let message = if current.is_deleted {
                    DELETED.to_string()
                } else {
                    format!("provider import key points to {}, not {}", current.record_type(), row.record_type)
                };
                staging.add_conflict(&match_key, Some(&current.record_id), &message)?;
                staging.decide(
                    row.position,
                    &Decision {
                        record_id: &current.record_id,
                        data_json: &data_json,
                        match_key: &match_key,
                        collision_group: None,
                        action: Action::Conflict,
                        current_revision: None,
                    },
                )?;
                continue;
            }
            let action =
                if same_logical_data(&current.data.to_value(), &data) { Action::Matching } else { Action::Changed };
            staging.decide(
                row.position,
                &Decision {
                    record_id: &current.record_id,
                    data_json: &data_json,
                    match_key: &match_key,
                    collision_group: collision_group.as_deref(),
                    action,
                    current_revision: Some(current.current_revision),
                },
            )?;
        }
        staging.commit()?;
    }
    for match_key in staging.changed_collision_groups()? {
        staging.add_conflict(&match_key, None, AMBIGUOUS_GROUP)?;
    }
    drop(read_tx);
    Ok(revision)
}

/// An existing source with the same writer, route, and sensor, deleted or
/// not, as the 2.1.0 helper found it. Returns its ID and whether it is deleted.
fn keyed_existing_source(connection: &rusqlite::Connection, data: &SourceData) -> Result<Option<(String, bool)>> {
    Ok(connection
        .query_row(
            "SELECT sources.record_id, records.is_deleted FROM sources
               JOIN records ON records.record_id = sources.record_id
              WHERE sources.data_writer = ?1
                AND sources.data_route = ?2
                AND COALESCE(sources.sensor_type, '') = COALESCE(?3, '')",
            params![data.data_writer, data.data_route, data.sensor_type],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
        )
        .optional()?)
}

/// `data` with the stored record's `received_at`, for comparing a keyed
/// candidate with what is stored (D182).
fn with_received_at(mut data: Value, stored: &Value) -> Value {
    if let (Value::Object(map), Some(received)) = (&mut data, stored.get("received_at")) {
        map.insert("received_at".into(), received.clone());
    }
    data
}

/// Decide a keyed route's candidates against the live database. Returns the
/// database revision the decisions were read at and the sources to create.
fn match_keyed(
    store: &Store,
    route: &Route,
    policy: KeyedPolicy,
    received_at: &str,
    staging: &mut Staging,
    sources: &[SourceEntry],
    report: &mut RouteReport,
) -> Result<(u64, Vec<(String, SourceData)>)> {
    enum Resolved {
        Ready(String),
        Deleted(String),
    }
    let read_tx = store.connection().unchecked_transaction()?;
    let connection: &rusqlite::Connection = &read_tx;
    let revision = store::database_revision(connection)?;

    let mut resolved: HashMap<String, Resolved> = HashMap::new();
    let mut new_sources = Vec::new();
    for source in sources {
        let entry = match keyed_existing_source(connection, &source.data)? {
            Some((record_id, false)) => Resolved::Ready(record_id),
            Some((record_id, true)) => Resolved::Deleted(record_id),
            None => {
                let data = RecordData::Source(source.data.clone()).to_value();
                check_record(&source.provisional_id, received_at, RecordType::Source, &data)?;
                new_sources.push((source.provisional_id.clone(), source.data.clone()));
                Resolved::Ready(source.provisional_id.clone())
            }
        };
        resolved.insert(source.provisional_id.clone(), entry);
    }
    let live = |sources: &Vec<String>| sources.iter().any(|id| matches!(resolved.get(id), Some(Resolved::Ready(_))));
    for (field, sources) in std::mem::take(&mut report.understood_under) {
        if live(&sources) {
            report.understood.insert(field);
        }
    }
    for (field, sources) in std::mem::take(&mut report.preserved_under) {
        if live(&sources) {
            report.preserved.insert(field);
        }
    }

    let mut deleted_source_groups = std::collections::HashSet::new();
    let mut after = None;
    loop {
        let rows = staging.rows_after(after, IMPORT_BATCH_RECORDS)?;
        let Some(last) = rows.last() else {
            break;
        };
        after = Some((last.section, last.position));
        staging.begin()?;
        for row in rows {
            let label = row.group_label.clone().unwrap_or_default();
            let seq = row.group_seq.unwrap_or(-1);
            let Value::String(match_key) = json::parse(&row.key_parts)? else {
                return Err(Error::other("A keyed route staged a key that is not plain text"));
            };
            let mut decision = Decision {
                record_id: &row.record_id,
                data_json: &row.data_json,
                match_key: &match_key,
                collision_group: None,
                action: Action::LeftOut,
                current_revision: None,
            };
            let mut data = json::parse(&row.data_json)?;
            if let Some(provisional) = &row.source_id {
                match resolved.get(provisional) {
                    Some(Resolved::Ready(final_id)) => {
                        if final_id != provisional {
                            if let Value::Object(map) = &mut data {
                                map.insert("source_id".into(), Value::String(final_id.clone()));
                            }
                        }
                    }
                    Some(Resolved::Deleted(source_id)) => {
                        if deleted_source_groups.insert(seq) {
                            report.left_out.push((seq, format!("{label}: its source {source_id} is deleted")));
                        }
                        staging.decide(row.position, &decision)?;
                        continue;
                    }
                    None => return Err(Error::other("A staged candidate names an unknown source")),
                }
            }
            let data_json = json::to_string(&data);
            decision.data_json = &data_json;

            if let Some((first_data, first_id, first_action)) = staging.first_with_key(&match_key, row.position)? {
                if !same_logical_data(&json::parse(&first_data)?, &data) {
                    return Err(Error::refused(format!("{DUPLICATE_DIFFERENT}: {match_key}")));
                }
                // A repeat of a left-out candidate is left out with it.
                decision.record_id = &first_id;
                decision.action = if first_action == Some(Action::LeftOut) { Action::LeftOut } else { Action::Repeat };
                staging.decide(row.position, &decision)?;
                continue;
            }

            let Some(current) = record_by_import_key(connection, &route.match_key_method, &match_key)? else {
                decision.action = Action::New;
                staging.decide(row.position, &decision)?;
                continue;
            };
            if current.record_type().as_str() != row.record_type {
                return Err(Error::refused(format!(
                    "import key {match_key} points to {}, not {}",
                    current.record_type(),
                    row.record_type
                )));
            }
            decision.record_id = &current.record_id;
            if current.is_deleted {
                report.left_out.push((seq, format!("{label}: the stored record {} is deleted", current.record_id)));
                staging.decide(row.position, &decision)?;
                continue;
            }
            let stored = current.data.to_value();
            let mut next = data;
            if policy.merge {
                let mut merged = stored.clone();
                if let (Value::Object(base), Value::Object(fields)) = (&mut merged, next) {
                    for (key, value) in fields {
                        base.insert(key, value);
                    }
                }
                next = merged;
            }
            if policy.keep_received_at {
                next = with_received_at(next, &stored);
            }
            let unchanged =
                json::to_sorted_string(&with_received_at(next.clone(), &stored)) == json::to_sorted_string(&stored);
            let record_type = current.record_type();
            let next = RecordData::decode(record_type, &next)?.to_value();
            check_record(&current.record_id, &current.created_at, record_type, &next)?;
            let next_json = json::to_string(&next);
            decision.data_json = &next_json;
            decision.action = if unchanged { Action::Matching } else { Action::Changed };
            decision.current_revision = Some(current.current_revision);
            staging.decide(row.position, &decision)?;
        }
        staging.commit()?;
    }
    drop(read_tx);
    Ok((revision, new_sources))
}

struct RecordList<'a> {
    staging: &'a Staging,
    action: Action,
}

impl Serialize for RecordList<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        let mut after = None;
        loop {
            let (page, last) = self
                .staging
                .planned_page(&[self.action], after, IMPORT_BATCH_RECORDS)
                .map_err(serde::ser::Error::custom)?;
            if page.is_empty() {
                break;
            }
            for record in page {
                seq.serialize_element(&reference(record, self.action))?;
            }
            after = last;
        }
        seq.end()
    }
}

struct SkippedMap<'a>(&'a [(String, Skipped)]);

impl Serialize for SkippedMap<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (reason, skipped) in self.0 {
            map.serialize_entry(reason, skipped)?;
        }
        map.end()
    }
}

fn reference(record: PlannedRecord, action: Action) -> PlannedRecordRef {
    PlannedRecordRef {
        record_id: record.record_id,
        record_type: record.record_type,
        current_revision: if action == Action::Changed { record.current_revision } else { None },
    }
}

impl Serialize for ProviderPreview {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        if self.route.report != ReportKind::Provider {
            let mut value = self.report_value(false).map_err(serde::ser::Error::custom)?;
            if let Value::Object(map) = &mut value {
                map.insert("write_confirmed".into(), Value::Bool(false));
            }
            return value.serialize(serializer);
        }
        let mut out = serializer.serialize_struct("ProviderPreview", 10)?;
        out.serialize_field("provider", &self.provider)?;
        out.serialize_field("imported", &self.imported)?;
        out.serialize_field("destination_database_revision", &self.destination_database_revision)?;
        out.serialize_field("new_records", &RecordList { staging: &self.staging, action: Action::New })?;
        out.serialize_field("matching_records", &RecordList { staging: &self.staging, action: Action::Matching })?;
        out.serialize_field("changed_records", &RecordList { staging: &self.staging, action: Action::Changed })?;
        out.serialize_field("conflicts", &self.conflicts)?;
        out.serialize_field("skipped", &SkippedMap(&self.skipped))?;
        out.serialize_field("route_points_stored", &self.route_points_stored)?;
        out.serialize_field("write_confirmed", &false)?;
        out.end()
    }
}

impl ProviderPreview {
    pub fn new_record_count(&self) -> u64 {
        self.counts[0]
    }

    pub fn matching_record_count(&self) -> u64 {
        self.counts[1]
    }

    pub fn changed_record_count(&self) -> u64 {
        self.counts[2]
    }

    /// The import time this plan stores.
    pub fn received_at(&self) -> &str {
        &self.received_at
    }

    /// The route this plan imports.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// What a keyed route understood, kept, and left out.
    pub fn report(&self) -> &RouteReport {
        &self.report
    }

    /// The record each written or repeated candidate names, in plan order.
    /// A keyed route lists these as `record_ids` once its plan is written.
    pub fn record_ids(&self) -> Result<Vec<String>> {
        self.staging.record_ids()
    }

    /// The result 2.1.0 printed for a keyed route, without `write_confirmed`.
    ///
    /// Before the write, `record_ids` is empty, as the 2.1.0 preview printed
    /// it. After a confirmed write, pass `written` to list every record the
    /// import names: new, changed, and unchanged ones, in input order.
    pub fn report_value(&self, written: bool) -> Result<Value> {
        let record_ids = if written { self.record_ids()? } else { Vec::new() };
        let ids = Value::Array(record_ids.into_iter().map(Value::String).collect());
        let list = |items: &std::collections::BTreeSet<String>| {
            Value::Array(items.iter().cloned().map(Value::String).collect())
        };
        let left_out = Value::Array(self.report.left_out().into_iter().map(Value::String).collect());
        let mut out = json::Map::new();
        match &self.route.report {
            ReportKind::Provider => return Ok(json::to_value(self)),
            ReportKind::Shared { source } => {
                out.insert("source".into(), Value::String(source.clone()));
                out.insert("record_ids".into(), ids);
                out.insert("understood".into(), list(&self.report.understood));
                out.insert("preserved".into(), list(&self.report.preserved));
                out.insert("left_out".into(), left_out);
            }
            ReportKind::Pull { source } => {
                out.insert("source".into(), Value::String(source.clone()));
                out.insert("record_ids".into(), ids);
                out.insert("understood".into(), list(&self.report.understood));
                out.insert("preserved".into(), list(&self.report.preserved));
                if !self.report.left_out.is_empty() {
                    out.insert("left_out".into(), left_out);
                }
            }
            ReportKind::Mapped { mapping } => {
                let mut outcome = json::Map::new();
                outcome.insert("new".into(), Value::from(self.counts[0]));
                outcome.insert("changed".into(), Value::from(self.counts[2]));
                outcome.insert("unchanged".into(), Value::from(self.counts[1]));
                out.insert("mapping".into(), Value::String(mapping.clone()));
                out.insert("record_ids".into(), ids);
                out.insert("outcome".into(), Value::Object(outcome));
                out.insert("understood".into(), list(&self.report.understood));
                out.insert("preserved".into(), list(&self.report.preserved));
                out.insert("left_out".into(), left_out);
            }
        }
        Ok(Value::Object(out))
    }

    /// The match key method of every planned key.
    pub fn match_key_method(&self) -> &str {
        &self.match_key_method
    }

    fn references(&self, action: Action) -> Result<Vec<PlannedRecordRef>> {
        Ok(self.staging.planned(&[action])?.into_iter().map(|record| reference(record, action)).collect())
    }

    pub fn new_records(&self) -> Result<Vec<PlannedRecordRef>> {
        self.references(Action::New)
    }

    pub fn matching_records(&self) -> Result<Vec<PlannedRecordRef>> {
        self.references(Action::Matching)
    }

    pub fn changed_records(&self) -> Result<Vec<PlannedRecordRef>> {
        self.references(Action::Changed)
    }

    /// Every new, matching, and changed record with its key and complete data,
    /// in plan order.
    pub fn planned(&self) -> Result<Vec<PlannedRecord>> {
        self.staging.planned(&[Action::New, Action::Matching, Action::Changed])
    }

    /// Write the plan in one transaction.
    ///
    /// Refused when the plan has conflicts, belongs to another database, or is
    /// stale: the database revision moved, a corrected record's revision
    /// moved, or a match key now names another record. A plan with nothing
    /// to write leaves the database revision unchanged.
    pub fn apply(&self, store: &mut Store) -> Result<ProviderImportResult> {
        if !self.conflicts.is_empty() {
            return Err(Error::refused("Provider import has unresolved conflicts"));
        }
        if store.metadata().database_id != self.database_id {
            return Err(Error::stale("The import preview was built for another database; preview it again"));
        }
        let layout = store.layout();
        let changed_by = RecordActor::Tool { tool: "ath".into(), tool_version: ATHLETIC_STANDARD_VERSION.to_string() };
        let correction_basis = match self.route.policy {
            MatchPolicy::Provider => BecameKnownAtBasis::ProviderImport,
            MatchPolicy::Keyed(policy) => policy.correction_basis,
        };
        let database_revision = store.transaction(|tx| {
            let revision = store::database_revision(tx)?;
            if revision != self.destination_database_revision {
                return Err(Error::stale("Database changed after provider import preview; preview it again"));
            }
            let mut wrote = false;
            if !self.new_sources.is_empty() {
                let mut changes = Changes::default();
                for (record_id, data) in &self.new_sources {
                    changes.creates.push(NewRecord {
                        data: RecordData::Source(data.clone()),
                        extensions: None,
                        record_id: Some(record_id.clone()),
                        created_at: Some(self.received_at.clone()),
                        became_known_at: Some(self.received_at.clone()),
                        became_known_at_basis: BecameKnownAtBasis::ProviderImport,
                        changed_by: changed_by.clone(),
                    });
                }
                wrote = true;
                write::write_in(tx, layout, changes)?;
            }
            let mut after = None;
            loop {
                let (page, last) = self.staging.planned_page(
                    &[Action::New, Action::Matching, Action::Changed],
                    after,
                    IMPORT_BATCH_RECORDS,
                )?;
                if page.is_empty() {
                    break;
                }
                after = last;
                let mut changes = Changes::default();
                for record in page {
                    let record_type = RecordType::parse(&record.record_type)
                        .ok_or_else(|| Error::other(format!("Unknown staged record type {}", record.record_type)))?;
                    match record.action {
                        Action::New => changes.creates.push(NewRecord {
                            data: RecordData::decode(record_type, &json::parse(&record.data_json)?)?,
                            extensions: None,
                            record_id: Some(record.record_id.clone()),
                            created_at: Some(self.received_at.clone()),
                            became_known_at: Some(self.received_at.clone()),
                            became_known_at_basis: BecameKnownAtBasis::ProviderImport,
                            changed_by: changed_by.clone(),
                        }),
                        Action::Changed => changes.corrections.push(Correction {
                            record_id: record.record_id.clone(),
                            expected_revision: record.current_revision.unwrap_or(0),
                            data: RecordData::decode(record_type, &json::parse(&record.data_json)?)?,
                            extensions: ExtensionsUpdate::Keep,
                            changed_by: changed_by.clone(),
                            became_known_at: Some(self.received_at.clone()),
                            became_known_at_basis: correction_basis,
                            stale_style: StaleStyle::Preview,
                        }),
                        Action::Matching | Action::Conflict | Action::Repeat | Action::LeftOut => {}
                    }
                    let owner: Option<String> = tx
                        .prepare_cached(
                            "SELECT record_id FROM record_import_keys WHERE match_key_method = ?1 AND match_key = ?2",
                        )?
                        .query_row(params![self.match_key_method, record.match_key], |row| row.get(0))
                        .optional()?;
                    match owner {
                        None => changes.import_keys.push(RecordImportKey {
                            record_id: record.record_id.clone(),
                            match_key_method: self.match_key_method.clone(),
                            match_key: record.match_key.clone(),
                            imported_at: self.received_at.clone(),
                        }),
                        Some(owner) if owner == record.record_id => {}
                        Some(_) => return Err(Error::stale("Provider import key changed after preview")),
                    }
                }
                if !changes.is_empty() {
                    wrote = true;
                    write::write_in(tx, layout, changes)?;
                }
            }
            let revision = if wrote { write::advance_database_revision(tx)? } else { revision };
            crate::package::failpoint!("provider.before_commit");
            Ok(revision)
        })?;
        crate::package::failpoint!("provider.after_commit");
        Ok(ProviderImportResult {
            database_revision,
            new_record_count: self.counts[0],
            changed_record_count: self.counts[2],
            matching_record_count: self.counts[1],
        })
    }
}
