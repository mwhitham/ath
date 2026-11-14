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

/// The history rules shared by the main document and series files.
fn validate_complete_record_history(value: &Value, issues: &mut Vec<Issue>) {
    let records = array(value, "records");
    let revisions = array(value, "record_revisions");
    let deletions = array(value, "record_deletions");
    let import_keys = array(value, "record_import_keys");

    let mut records_by_id: HashMap<&str, (&Value, usize)> = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        let id = text(record, "record_id");
        if records_by_id.contains_key(id) {
            issue(
                issues,
                at(&["records"], index, &["record_id"]),
                "A package cannot contain the same current record twice",
            );
            continue;
        }
        records_by_id.insert(id, (record, index));
    }

    let mut revisions_by_record: HashMap<&str, Vec<&Value>> = HashMap::new();
    let mut revision_keys: HashSet<(String, String)> = HashSet::new();
    for (index, revision) in revisions.iter().enumerate() {
        let id = text(revision, "record_id");
        let key = (id.to_string(), json::to_string(revision.get("revision").unwrap_or(&Value::Null)));
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
            Some((owner, _)) if text(owner, "record_type") != text(revision, "record_type") => issue(
                issues,
                at(&["record_revisions"], index, &["record_type"]),
                "A revision must keep its record type",
            ),
            _ => {}
        }
        revisions_by_record.entry(id).or_default().push(revision);
    }

    let mut deletion_by_record: HashMap<&str, &Value> = HashMap::new();
    for (index, deletion) in deletions.iter().enumerate() {
        let id = text(deletion, "record_id");
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
    for record in records {
        let id = text(record, "record_id");
        if !seen.insert(id) {
            continue;
        }
        let (record, index) = records_by_id[id];
        let current = int(record, "current_revision").unwrap_or(f64::NAN);
        let mut group: Vec<&Value> = revisions_by_record.get(id).cloned().unwrap_or_default();
        group.sort_by(|left, right| {
            let left = int(left, "revision").unwrap_or(f64::NAN);
            let right = int(right, "revision").unwrap_or(f64::NAN);
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
            if int(revision, "revision") != Some((position + 1) as f64) {
                issue(
                    issues,
                    at(&["records"], index, &["current_revision"]),
                    "Record revision numbers must be complete and consecutive",
                );
            }
        }
        if let Some(latest) = group.last() {
            let same_revision = int(latest, "revision") == Some(current);
            let same_data = match (latest.get("data"), record.get("data")) {
                (Some(left), Some(right)) => {
                    json::to_string(left) == json::to_string(right) || json::semantic_eq(left, right)
                }
                _ => false,
            };
            if !same_revision || !same_data {
                issue(
                    issues,
                    at(&["records"], index, &["data"]),
                    "The current record must equal its current retained revision",
                );
            }
        }
        let deleted = record.get("is_deleted").and_then(Value::as_bool).unwrap_or(false);
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
            if int(deletion, "record_revision") != Some(current) {
                issue(
                    issues,
                    at(&["records"], index, &["current_revision"]),
                    "A deletion record must name the current retained revision",
                );
            }
        }
    }

    let mut owners: HashMap<(String, String), &str> = HashMap::new();
    for (index, key) in import_keys.iter().enumerate() {
        let id = text(key, "record_id");
        if !records_by_id.contains_key(id) {
            issue(
                issues,
                at(&["record_import_keys"], index, &["record_id"]),
                "A provider import key needs its current record",
            );
        }
        let pair = (text(key, "match_key_method").to_string(), text(key, "match_key").to_string());
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
    validate_complete_record_history(value, issues);
    let start = js_date_parse(text(value, "start_at"));
    let end = js_date_parse(text(value, "end_at"));
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            custom(issues, &["end_at"], "Series file end cannot precede its start");
        }
    }
    let source_id = text(value, "source_id");
    let quantity = text(value, "quantity");
    for (index, record) in array(value, "records").iter().enumerate() {
        let data = record.get("data").unwrap_or(&Value::Null);
        if text(data, "source_id") != source_id || text(data, "quantity") != quantity {
            issue(issues, at(&["records"], index, &[]), "Every sample must match the series source and quantity");
        }
        let recorded = js_date_parse(text(data, "recorded_at"));
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
