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
//! These are the rules of `src/db/provider-import.ts` in 2.1.0.

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
use super::staging::{Action, Decision, PlannedRecord, Staging};

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
        }
    }
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

/// The reader for a detected format, when this build has one.
pub fn reader_for(format: ExportFormat) -> Option<&'static dyn ProviderReader> {
    match format {
        ExportFormat::Whoop => Some(&super::whoop::WhoopReader),
        ExportFormat::Oura => Some(&super::oura::OuraReader),
        ExportFormat::Apple => None,
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

/// Where a reader sends what it found.
pub struct CandidateSink<'a> {
    route: &'a Route,
    received_at: &'a str,
    staging: &'a mut Staging,
    sources: Vec<SourceEntry>,
    skipped: SkipList,
    pending: usize,
}

impl<'a> CandidateSink<'a> {
    fn new(route: &'a Route, received_at: &'a str, staging: &'a mut Staging) -> Result<Self> {
        staging.begin()?;
        Ok(CandidateSink { route, received_at, staging, sources: Vec::new(), skipped: Vec::new(), pending: 0 })
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
        self.staging.insert(candidate.section as i64, &record_id, &json::to_string(&data), &candidate, None)?;
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

    fn finish(self) -> Result<(Vec<SourceEntry>, SkipList)> {
        self.staging.commit()?;
        let mut skipped = self.skipped;
        skipped.sort_by(|(left, _), (right, _)| locale_compare(left, right));
        Ok((self.sources, skipped))
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
    let received_at = options.received_at.clone().unwrap_or_else(time::now_utc);
    if !time::is_utc_timestamp(&received_at) {
        return Err(Error::Invalid(ValidationError::single(
            vec![PathSegment::Key("received_at".into())],
            "Invalid ISO datetime",
        )));
    }
    let route = reader.route(export);
    let mut staging = Staging::create()?;
    let (sources, skipped) = {
        let mut sink = CandidateSink::new(&route, &received_at, &mut staging)?;
        reader.read(export, &mut sink)?;
        sink.finish()?
    };
    let destination_database_revision = match_plan(store, &route, &received_at, &mut staging, &sources)?;
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
        staging.insert(0, &record_id, &json::to_string(&data), &candidate, existing.as_deref())?;
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

            if let Some(first) = staging.first_with_key(&match_key, row.position)? {
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
        let database_revision = store.transaction(|tx| {
            let revision = store::database_revision(tx)?;
            if revision != self.destination_database_revision {
                return Err(Error::stale("Database changed after provider import preview; preview it again"));
            }
            let mut wrote = false;
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
                            became_known_at_basis: BecameKnownAtBasis::ProviderImport,
                            stale_style: StaleStyle::Preview,
                        }),
                        Action::Matching | Action::Conflict => {}
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
            if wrote {
                write::advance_database_revision(tx)
            } else {
                Ok(revision)
            }
        })?;
        Ok(ProviderImportResult {
            database_revision,
            new_record_count: self.counts[0],
            changed_record_count: self.counts[2],
            matching_record_count: self.counts[1],
        })
    }
}
