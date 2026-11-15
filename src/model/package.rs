//! The portable JSON package: the main document, measurement-series files, and
//! the export manifest.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::common::*;
use super::data::SeriesQuantity;
use super::record::*;
use super::schema::{
    custom, leak, nonnegative_int, Field, Issue, IssueCode, Kind, PathSegment, Schema, SchemaRef, Shape,
};
use crate::json::{self, Value};
use crate::{model_enum, model_object};

/// The software, record, and database version (D152).
pub const ATHLETIC_STANDARD_VERSION: &str = "2.1.1";

/// Record files this software can read. New writes use `ATHLETIC_STANDARD_VERSION`.
pub const ACCEPTED_RECORD_VERSIONS: &[&str] = &["1.0.0", "2.0.0", "2.1.0", "2.1.1"];

/// Package versions `ath import` restores. 2.1.x and 2.0.0 packages share the
/// same fields; later minor versions only add optional fields.
pub const IMPORTABLE_PACKAGE_VERSIONS: &[&str] = &["2.0.0", "2.1.0", "2.1.1"];

model_enum! {
    pub enum RecordFormatVersion {
        V1_0_0 = "1.0.0",
        V2_0_0 = "2.0.0",
        V2_1_0 = "2.1.0",
        V2_1_1 = "2.1.1",
    }
}

model_object! {
    pub struct MeasurementSeriesFileReference {
        file: String = package_relative_path(),
        sha256: String = sha256(),
        source_id: String = record_id(),
        quantity: SeriesQuantity,
        start_at: String = utc_timestamp(),
        end_at: String = utc_timestamp(),
        sample_count: u64 = nonnegative_int(),
        revision_count: u64 = nonnegative_int(),
        deletion_count: u64 = nonnegative_int(),
        import_key_count: u64 = nonnegative_int(),
    }
}

model_object! {
    pub struct ManifestFile {
        file: String = package_relative_path(),
        sha256: String = sha256(),
        byte_count: u64 = nonnegative_int(),
    }
}

model_object! {
    pub struct ManifestRecordCount {
        record_type: RecordType,
        current_record_count: u64 = nonnegative_int(),
        revision_count: u64 = nonnegative_int(),
        deletion_count: u64 = nonnegative_int(),
        import_key_count: u64 = nonnegative_int(),
    }
}

model_object! {
    /// The manifest is written last. It names every package file and its hash.
    pub struct ExportManifest {
        athleticstandard_version: RecordFormatVersion,
        source_database_id: String = database_id(),
        database_revision: u64 = nonnegative_int(),
        exported_at?: String = utc_timestamp(),
        files: Vec<ManifestFile>,
        record_counts: Vec<ManifestRecordCount>,
        erasure_marker_count: u64 = nonnegative_int(),
    }
}

fn field(name: &'static str, schema: SchemaRef) -> Field {
    Field { name, schema, optional: false }
}

/// The main package document shape.
pub fn package_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
        Schema::new(Kind::Object {
            fields: vec![
                field("athleticstandard_version", RecordFormatVersion::schema()),
                field("database_id", database_id()),
                field("database_revision", nonnegative_int()),
                field("records", leak(Schema::new(Kind::Array(non_sample_record_schema())))),
                field("record_revisions", leak(Schema::new(Kind::Array(non_sample_revision_schema())))),
                field("record_deletions", Vec::<RecordDeletion>::schema()),
                field("record_erasure_markers", Vec::<RecordErasureMarker>::schema()),
                field("record_import_keys", Vec::<RecordImportKey>::schema()),
                field("measurement_series_files", Vec::<MeasurementSeriesFileReference>::schema()),
            ],
        })
        .refine(refine_package)
    });
    &SCHEMA
}

/// One measurement-series file shape.
pub fn measurement_series_file_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
        Schema::new(Kind::Object {
            fields: vec![
                field("athleticstandard_version", RecordFormatVersion::schema()),
                field("source_id", record_id()),
                field("quantity", SeriesQuantity::schema()),
                field("start_at", utc_timestamp()),
                field("end_at", utc_timestamp()),
                field("records", leak(Schema::new(Kind::Array(measurement_sample_record_schema())))),
                field("record_revisions", leak(Schema::new(Kind::Array(measurement_sample_revision_schema())))),
                field("record_deletions", Vec::<RecordDeletion>::schema()),
                field("record_import_keys", Vec::<RecordImportKey>::schema()),
            ],
        })
        .refine(refine_series_file)
    });
    &SCHEMA
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn int(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn issue(issues: &mut Vec<Issue>, path: Vec<PathSegment>, message: &str) {
    issues.push(Issue { path, code: IssueCode::Custom, message: message.into() });
}

fn at(parts: &[&str], index: usize, tail: &[&str]) -> Vec<PathSegment> {
    let mut path: Vec<PathSegment> = parts.iter().map(|part| PathSegment::Key((*part).into())).collect();
    path.push(PathSegment::Index(index));
    path.extend(tail.iter().map(|part| PathSegment::Key((*part).into())));
    path
}

/// What the history rules read from one current record of a package file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecordFacts {
    pub record_id: String,
    pub record_type: String,
    pub current_revision: Option<f64>,
    pub is_deleted: bool,
    /// The data as its record type outputs it, as JSON text.
    pub data_text: Option<String>,
    /// `data.source_id`, `data.quantity`, and `data.recorded_at`, which the
    /// series rules read.
    pub data_source_id: String,
    pub data_quantity: String,
    pub data_recorded_at: String,
}

impl RecordFacts {
    pub(crate) fn of(record: &Value) -> Self {
        let data = record.get("data").unwrap_or(&Value::Null);
        RecordFacts {
            record_id: text(record, "record_id").to_string(),
            record_type: text(record, "record_type").to_string(),
            current_revision: int(record, "current_revision"),
            is_deleted: record.get("is_deleted").and_then(Value::as_bool).unwrap_or(false),
            data_text: data_text(record),
            data_source_id: text(data, "source_id").to_string(),
            data_quantity: text(data, "quantity").to_string(),
            data_recorded_at: text(data, "recorded_at").to_string(),
        }
    }
}

/// What the history rules read from one retained revision.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RevisionFacts {
    pub record_id: String,
    pub record_type: String,
    /// The `revision` value as JSON text, which identifies a repeated revision.
    pub revision_text: String,
    pub revision: Option<f64>,
    pub data_text: Option<String>,
}

impl RevisionFacts {
    pub(crate) fn of(revision: &Value) -> Self {
        RevisionFacts {
            record_id: text(revision, "record_id").to_string(),
            record_type: text(revision, "record_type").to_string(),
            revision_text: json::to_string(revision.get("revision").unwrap_or(&Value::Null)),
            revision: int(revision, "revision"),
            data_text: data_text(revision),
        }
    }
}

/// What the history rules read from one deletion record.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeletionFacts {
    pub record_id: String,
    pub record_revision: Option<f64>,
}

/// What the history rules read from one provider import key.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImportKeyFacts {
    pub record_id: String,
    pub match_key_method: String,
    pub match_key: String,
}

/// The parts of a package file that the history rules read. A series file
/// can be reduced to these one item at a time, so its whole JSON tree need
/// not be held to check it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct HistoryFacts {
    pub records: Vec<RecordFacts>,
    pub revisions: Vec<RevisionFacts>,
    pub deletions: Vec<DeletionFacts>,
    pub import_keys: Vec<ImportKeyFacts>,
}

impl HistoryFacts {
    /// The facts of a package document or series file.
    pub(crate) fn of(value: &Value) -> Self {
        let mut facts = HistoryFacts {
            records: array(value, "records").iter().map(RecordFacts::of).collect(),
            revisions: array(value, "record_revisions").iter().map(RevisionFacts::of).collect(),
            ..HistoryFacts::default()
        };
        facts.add_deletions_and_import_keys(value);
        facts
    }

    /// Add the deletion records and import keys of a file.
    pub(crate) fn add_deletions_and_import_keys(&mut self, value: &Value) {
        self.deletions = array(value, "record_deletions")
            .iter()
            .map(|deletion| DeletionFacts {
                record_id: text(deletion, "record_id").to_string(),
                record_revision: int(deletion, "record_revision"),
            })
            .collect();
        self.import_keys = array(value, "record_import_keys")
            .iter()
            .map(|key| ImportKeyFacts {
                record_id: text(key, "record_id").to_string(),
                match_key_method: text(key, "match_key_method").to_string(),
                match_key: text(key, "match_key").to_string(),
            })
            .collect();
    }
}

/// The data of a record or revision as its record type outputs it, as JSON
/// text. The history rules compare this text: key order inside supplied
/// fields is part of the value, as in the published check.
fn data_text(value: &Value) -> Option<String> {
    let shape = RecordType::parse(text(value, "record_type")).map(data_schema)?;
    Some(json::to_string(&shape.output(value.get("data")?)))
}

/// The history rules shared by the main document and series files.
fn validate_complete_record_history(value: &Value, issues: &mut Vec<Issue>) {
    history_issues(&HistoryFacts::of(value), issues);
}

fn history_issues(facts: &HistoryFacts, issues: &mut Vec<Issue>) {
    let mut records_by_id: HashMap<&str, usize> = HashMap::new();
    for (index, record) in facts.records.iter().enumerate() {
        let id = record.record_id.as_str();
        if records_by_id.contains_key(id) {
            issue(
                issues,
                at(&["records"], index, &["record_id"]),
                "A package cannot contain the same current record twice",
            );
            continue;
        }
        records_by_id.insert(id, index);
    }

    let mut revisions_by_record: HashMap<&str, Vec<&RevisionFacts>> = HashMap::new();
    let mut revision_keys: HashSet<(&str, &str)> = HashSet::new();
    for (index, revision) in facts.revisions.iter().enumerate() {
        let id = revision.record_id.as_str();
        let key = (id, revision.revision_text.as_str());
        if revision_keys.contains(&key) {
            issue(
                issues,
                at(&["record_revisions"], index, &[]),
                "A package cannot contain the same record revision twice",
            );
        }
        revision_keys.insert(key);
        match records_by_id.get(id) {
            None => issue(
                issues,
                at(&["record_revisions"], index, &["record_id"]),
                "A retained revision needs its current record",
            ),
            Some(owner) if facts.records[*owner].record_type != revision.record_type => issue(
                issues,
                at(&["record_revisions"], index, &["record_type"]),
                "A revision must keep its record type",
            ),
            _ => {}
        }
        revisions_by_record.entry(id).or_default().push(revision);
    }

    let mut deletion_by_record: HashMap<&str, &DeletionFacts> = HashMap::new();
    for (index, deletion) in facts.deletions.iter().enumerate() {
        let id = deletion.record_id.as_str();
        if !records_by_id.contains_key(id) {
            issue(
                issues,
                at(&["record_deletions"], index, &["record_id"]),
                "A deletion record needs its retained current record",
            );
        }
        if deletion_by_record.contains_key(id) {
            issue(
                issues,
                at(&["record_deletions"], index, &["record_id"]),
                "A record can have only one current deletion record",
            );
        }
        deletion_by_record.insert(id, deletion);
    }

    // Records are checked in first-appearance order, as a JavaScript Map iterates.
    let mut seen: HashSet<&str> = HashSet::new();
    for record in &facts.records {
        let id = record.record_id.as_str();
        if !seen.insert(id) {
            continue;
        }
        let index = records_by_id[id];
        let record = &facts.records[index];
        let current = record.current_revision.unwrap_or(f64::NAN);
        let mut group: Vec<&RevisionFacts> = revisions_by_record.get(id).cloned().unwrap_or_default();
        group.sort_by(|left, right| {
            let left = left.revision.unwrap_or(f64::NAN);
            let right = right.revision.unwrap_or(f64::NAN);
            left.partial_cmp(&right).unwrap_or(std::cmp::Ordering::Equal)
        });
        if group.len() as f64 != current {
            issue(
                issues,
                at(&["records"], index, &["current_revision"]),
                "A complete package needs every revision through current_revision",
            );
        }
        for (position, revision) in group.iter().enumerate() {
            if revision.revision != Some((position + 1) as f64) {
                issue(
                    issues,
                    at(&["records"], index, &["current_revision"]),
                    "Record revision numbers must be complete and consecutive",
                );
            }
        }
        if let Some(latest) = group.last() {
            let same_revision = latest.revision == Some(current);
            let same_data =
                matches!((&latest.data_text, &record.data_text), (Some(left), Some(right)) if left == right);
            if !same_revision || !same_data {
                issue(
                    issues,
                    at(&["records"], index, &["data"]),
                    "The current record must equal its current retained revision",
                );
            }
        }
        let deleted = record.is_deleted;
        let deletion = deletion_by_record.get(id);
        if deleted && deletion.is_none() {
            issue(
                issues,
                at(&["records"], index, &["is_deleted"]),
                "A deleted current record needs its deletion record",
            );
        }
        if !deleted && deletion.is_some() {
            issue(issues, at(&["records"], index, &["is_deleted"]), "A current deletion record requires is_deleted");
        }
        if let Some(deletion) = deletion {
            if deletion.record_revision != Some(current) {
                issue(
                    issues,
                    at(&["records"], index, &["current_revision"]),
                    "A deletion record must name the current retained revision",
                );
            }
        }
    }

    // The published check joins method and key with U+0000, so two pairs that
    // join to the same text count as the same key.
    let mut owners: HashMap<String, &str> = HashMap::new();
    for (index, key) in facts.import_keys.iter().enumerate() {
        let id = key.record_id.as_str();
        if !records_by_id.contains_key(id) {
            issue(
                issues,
                at(&["record_import_keys"], index, &["record_id"]),
                "A provider import key needs its current record",
            );
        }
        let pair = format!("{}\u{0}{}", key.match_key_method, key.match_key);
        if let Some(existing) = owners.get(&pair) {
            if *existing != id {
                issue(
                    issues,
                    at(&["record_import_keys"], index, &["match_key"]),
                    "One provider import key cannot identify two records",
                );
            }
        }
        owners.insert(pair, id);
    }
}

fn refine_package(value: &Value, issues: &mut Vec<Issue>) {
    validate_complete_record_history(value, issues);
    let mut files: HashSet<&str> = HashSet::new();
    for (index, file) in array(value, "measurement_series_files").iter().enumerate() {
        let name = text(file, "file");
        if !files.insert(name) {
            issue(
                issues,
                at(&["measurement_series_files"], index, &["file"]),
                "A package cannot reference the same measurement-series file twice",
            );
        }
    }
    let record_ids: HashSet<&str> = array(value, "records").iter().map(|record| text(record, "record_id")).collect();
    for (index, marker) in array(value, "record_erasure_markers").iter().enumerate() {
        if record_ids.contains(text(marker, "record_id")) {
            issue(
                issues,
                at(&["record_erasure_markers"], index, &["record_id"]),
                "An erased record cannot remain in current package records",
            );
        }
    }
}

fn refine_series_file(value: &Value, issues: &mut Vec<Issue>) {
    series_issues(value, &HistoryFacts::of(value), issues);
}

/// The series-file rules: complete history, then the time bounds, then each
/// sample's source, quantity, and time. `file` supplies the declared
/// `source_id`, `quantity`, `start_at`, and `end_at`; `facts` hold the items.
pub(crate) fn series_issues(file: &Value, facts: &HistoryFacts, issues: &mut Vec<Issue>) {
    history_issues(facts, issues);
    let start = js_date_parse(text(file, "start_at"));
    let end = js_date_parse(text(file, "end_at"));
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            custom(issues, &["end_at"], "Series file end cannot precede its start");
        }
    }
    let source_id = text(file, "source_id");
    let quantity = text(file, "quantity");
    for (index, record) in facts.records.iter().enumerate() {
        if record.data_source_id != source_id || record.data_quantity != quantity {
            issue(issues, at(&["records"], index, &[]), "Every sample must match the series source and quantity");
        }
        let recorded = js_date_parse(&record.data_recorded_at);
        let outside = match (recorded, start, end) {
            (Some(recorded), Some(start), Some(end)) => recorded < start || recorded > end,
            _ => false,
        };
        if outside {
            issue(
                issues,
                at(&["records"], index, &["data", "recorded_at"]),
                "Every current sample must fall inside the series time bounds",
            );
        }
    }
}
