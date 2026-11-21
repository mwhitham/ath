//! Complete deterministic package export.
//!
//! Export reads one database snapshot. It writes measurement-series files,
//! attachment copies, the main document, and finally the manifest into a
//! private staging folder, then publishes the folder with one rename. An
//! existing destination is never replaced.
//!
//! Files are streamed: one record at a time is read, checked against its
//! published shape, and written. Memory holds one series file's record list
//! (at most `SERIES_CHUNK_RECORDS` short entries), never the samples
//! themselves. The bytes equal `JSON.stringify(stableValue(file), null, 2)`
//! plus a newline, as version 2.1.0 wrote them.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use super::attachments::{attachment_directory, check_attachment_bytes, stored_attachment_name, ATTACHMENT_DIRECTORY};
use super::{failpoint, MANIFEST_DOCUMENT, PACKAGE_DOCUMENT, SERIES_CHUNK_RECORDS};
use crate::error::{Error, Result};
use crate::files::{self, durable, durable::HashingFile, staging};
use crate::json::{self, Map, Value};
use crate::model::*;
use crate::store::{self, Store};

/// Export settings.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// An exact UTC time to write in the manifest. Without it the manifest has
    /// no time, so two exports of one database revision are byte-identical.
    pub exported_at: Option<String>,
}

/// The `ath export --json` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportResult {
    /// Absolute package folder.
    pub directory: String,
    pub database_id: String,
    pub database_revision: u64,
    pub manifest: ExportManifest,
}

/// Write a complete package into `output_directory`, which must not exist.
pub fn export_package(store: &Store, output_directory: &Path, options: &ExportOptions) -> Result<ExportResult> {
    if let Some(exported_at) = &options.exported_at {
        crate::model::common::utc_timestamp().validate(&Value::String(exported_at.clone()))?;
    }
    let output = files::absolute(output_directory)?;
    let parent = durable::parent_of(&output);
    let final_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::refused(format!("export directory needs a name: {}", output.display())))?
        .to_string();
    files::refuse_symbolic_link_components(&parent, false)?;
    staging::cleanup_stale_export_staging_directories(&parent, None)?;
    refuse_existing(&output)?;

    let staging_path =
        parent.join(staging::export_staging_directory_name(std::process::id(), time::now_millis(), &final_name));
    files::create_private_dir(&staging_path)?;
    let written = (|| {
        staging::write_export_staging_marker(&staging_path)?;
        let written = write_package(store, &staging_path, options.exported_at.clone())?;
        failpoint!("export.before_rename");
        publish(&staging_path, &output)?;
        Ok(written)
    })();
    let (database_id, database_revision, manifest) = match written {
        Ok(written) => written,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_path);
            return Err(error);
        }
    };
    failpoint!("export.after_rename");
    durable::remove_file_if_present(&output.join(staging::EXPORT_STAGING_MARKER))?;
    files::sync_directory(&output)?;
    files::sync_directory(&parent)?;
    Ok(ExportResult { directory: output.display().to_string(), database_id, database_revision, manifest })
}

fn refuse_existing(output: &Path) -> Result<()> {
    match fs::symlink_metadata(output) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::refused(format!("refusing export through a symbolic link: {}", output.display())))
        }
        Ok(_) => Err(Error::refused(format!("export directory already exists: {}", output.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Synchronize the staged package, then make it visible at its final name.
fn publish(staging_path: &Path, output: &Path) -> Result<()> {
    for child in ["series", ATTACHMENT_DIRECTORY] {
        let path = staging_path.join(child);
        if path.is_dir() {
            files::sync_directory(&path)?;
        }
    }
    files::sync_directory(staging_path)?;
    let parent = durable::parent_of(output);
    files::sync_directory(&parent)?;
    refuse_existing(output)?;
    fs::rename(staging_path, output)?;
    files::sync_directory(&parent)?;
    Ok(())
}

type Written = (String, u64, ExportManifest);

fn write_package(store: &Store, staging_path: &Path, exported_at: Option<String>) -> Result<Written> {
    let connection = store.connection();
    // One deferred read transaction: every query below sees the same snapshot.
    let snapshot = connection.unchecked_transaction()?;
    let (version, database_id): (String, String) = snapshot.query_row(
        "SELECT athleticstandard_version, database_id FROM database_metadata WHERE singleton_id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if version != ATHLETIC_STANDARD_VERSION {
        return Err(Error::other("database metadata is invalid during export"));
    }
    let database_revision = store::database_revision(&snapshot)?;
    let mut manifest_files: Vec<ManifestFile> = Vec::new();

    let series = write_series_files(&snapshot, staging_path, &mut manifest_files)?;
    failpoint!("export.after_series");
    write_attachments(&snapshot, &attachment_directory(store.path()), staging_path, &mut manifest_files)?;
    let erasure_marker_count =
        write_main_document(&snapshot, staging_path, &database_id, database_revision, &series, &mut manifest_files)?;
    failpoint!("export.before_manifest");

    manifest_files.sort_by(|left, right| json::compare_utf16(&left.file, &right.file));
    let manifest = ExportManifest {
        athleticstandard_version: RecordFormatVersion::current(),
        source_database_id: database_id.clone(),
        database_revision,
        exported_at,
        files: manifest_files,
        record_counts: record_counts(&snapshot)?,
        erasure_marker_count,
    };
    let value = json::to_value(&manifest);
    ExportManifest::schema().validate(&value)?;
    durable::write_owner_only_durable(
        &staging_path.join(MANIFEST_DOCUMENT),
        json::to_deterministic_pretty(&value).as_bytes(),
    )?;
    snapshot.rollback()?;
    Ok((database_id, database_revision, manifest))
}

// ---- Streaming JSON ---------------------------------------------------------

/// A package file written as a sorted, two-space-indented JSON object.
struct JsonDocument {
    file: HashingFile,
    keys: usize,
}

impl JsonDocument {
    fn create(path: &Path) -> Result<Self> {
        let mut file = HashingFile::create(path)?;
        file.write_all(b"{")?;
        Ok(JsonDocument { file, keys: 0 })
    }

    fn key(&mut self, key: &str) -> Result<()> {
        let mut text = String::from(if self.keys == 0 { "\n  " } else { ",\n  " });
        json::write_string(&mut text, key);
        text.push_str(": ");
        self.file.write_all(text.as_bytes())?;
        self.keys += 1;
        Ok(())
    }

    fn scalar(&mut self, key: &str, value: &Value) -> Result<()> {
        self.key(key)?;
        self.file.write_all(json::to_string(value).as_bytes())?;
        Ok(())
    }

    fn array(&mut self, key: &str) -> Result<ArrayWriter<'_>> {
        self.key(key)?;
        Ok(ArrayWriter { document: self, count: 0 })
    }

    fn finish(mut self) -> Result<(String, u64)> {
        self.file.write_all(if self.keys == 0 { b"}\n" } else { b"\n}\n" })?;
        Ok(self.file.finish()?)
    }
}

struct ArrayWriter<'a> {
    document: &'a mut JsonDocument,
    count: usize,
}

impl ArrayWriter<'_> {
    fn item(&mut self, value: &Value) -> Result<()> {
        let pretty = json::to_deterministic_pretty(value);
        let body = pretty.strip_suffix('\n').unwrap_or(&pretty).replace('\n', "\n    ");
        let lead: &[u8] = if self.count == 0 { b"[\n    " } else { b",\n    " };
        self.document.file.write_all(lead)?;
        self.document.file.write_all(body.as_bytes())?;
        self.count += 1;
        Ok(())
    }

    fn finish(self) -> Result<usize> {
        self.document.file.write_all(if self.count == 0 { b"[]" } else { b"\n  ]" })?;
        Ok(self.count)
    }
}

// ---- Rows to package values ---------------------------------------------------

fn parse_json_column(text: &str, column: &str) -> Result<Value> {
    json::parse(text).map_err(|error| Error::other(format!("{column} is not valid JSON: {error}")))
}

fn extensions_value(text: Option<String>) -> Result<Option<Value>> {
    match text {
        None => Ok(None),
        Some(text) => match parse_json_column(&text, "extensions_json")? {
            Value::Null => Ok(None),
            value @ Value::Object(_) => Ok(Some(value)),
            _ => Err(Error::other("extensions_json must be an object")),
        },
    }
}

const CURRENT_COLUMNS: &str = "records.record_id, records.record_type, records.current_revision,
    records.created_at, records.is_deleted, record_revisions.record_json, record_revisions.extensions_json";

/// A current record as the package writes it.
fn current_value(row: &Row<'_>) -> Result<(String, RecordType, Value)> {
    let record_id: String = row.get(0)?;
    let record_type_text: String = row.get(1)?;
    let record_type = RecordType::parse(&record_type_text)
        .ok_or_else(|| Error::other(format!("record {record_id} has unsupported record type {record_type_text}")))?;
    let mut map = Map::new();
    map.insert("record_id".into(), Value::String(record_id.clone()));
    map.insert("record_type".into(), Value::String(record_type_text));
    map.insert("current_revision".into(), Value::from(row.get::<_, i64>(2)?));
    map.insert("created_at".into(), Value::String(row.get(3)?));
    map.insert("is_deleted".into(), Value::Bool(row.get::<_, i64>(4)? == 1));
    map.insert("data".into(), parse_json_column(&row.get::<_, String>(5)?, "record_json")?);
    if let Some(extensions) = extensions_value(row.get(6)?)? {
        map.insert("extensions".into(), extensions);
    }
    let value = Value::Object(map);
    current_record_variant(record_type).validate(&value).map_err(|error| located(error, &record_id))?;
    Ok((record_id, record_type, value))
}

const REVISION_COLUMNS: &str = "record_revisions.record_id, record_revisions.record_type, record_revisions.revision,
    record_revisions.previous_revision, record_revisions.became_known_at, record_revisions.became_known_at_basis,
    record_revisions.changed_by_json, record_revisions.record_json, record_revisions.extensions_json";

fn revision_value(row: &Row<'_>) -> Result<(String, RecordType, u64, Value)> {
    let record_id: String = row.get(0)?;
    let record_type_text: String = row.get(1)?;
    let record_type = RecordType::parse(&record_type_text)
        .ok_or_else(|| Error::other(format!("record {record_id} has unsupported record type {record_type_text}")))?;
    let revision: i64 = row.get(2)?;
    let previous: Option<i64> = row.get(3)?;
    let mut map = Map::new();
    map.insert("record_id".into(), Value::String(record_id.clone()));
    map.insert("record_type".into(), Value::String(record_type_text));
    map.insert("revision".into(), Value::from(revision));
    map.insert("previous_revision".into(), previous.map_or(Value::Null, Value::from));
    map.insert("became_known_at".into(), Value::String(row.get(4)?));
    map.insert("became_known_at_basis".into(), Value::String(row.get(5)?));
    map.insert("changed_by".into(), parse_json_column(&row.get::<_, String>(6)?, "changed_by_json")?);
    map.insert("data".into(), parse_json_column(&row.get::<_, String>(7)?, "record_json")?);
    if let Some(extensions) = extensions_value(row.get(8)?)? {
        map.insert("extensions".into(), extensions);
    }
    let value = Value::Object(map);
    revision_variant(record_type).validate(&value).map_err(|error| located(error, &record_id))?;
    Ok((record_id, record_type, revision as u64, value))
}

const DELETION_COLUMNS: &str = "record_deletions.record_id, record_deletions.record_revision,
    record_deletions.deleted_at, record_deletions.deleted_by_json";

fn deletion_value(row: &Row<'_>) -> Result<(String, u64, Value)> {
    let record_id: String = row.get(0)?;
    let revision: i64 = row.get(1)?;
    let mut map = Map::new();
    map.insert("record_id".into(), Value::String(record_id.clone()));
    map.insert("record_revision".into(), Value::from(revision));
    map.insert("deleted_at".into(), Value::String(row.get(2)?));
    map.insert("deleted_by".into(), parse_json_column(&row.get::<_, String>(3)?, "deleted_by_json")?);
    let value = Value::Object(map);
    RecordDeletion::schema().validate(&value).map_err(|error| located(error, &record_id))?;
    Ok((record_id, revision as u64, value))
}

const IMPORT_KEY_COLUMNS: &str = "record_import_keys.record_id, record_import_keys.match_key_method,
    record_import_keys.match_key, record_import_keys.imported_at";

fn import_key_value(row: &Row<'_>) -> Result<(String, Value)> {
    let record_id: String = row.get(0)?;
    let mut map = Map::new();
    map.insert("record_id".into(), Value::String(record_id.clone()));
    map.insert("match_key_method".into(), Value::String(row.get(1)?));
    map.insert("match_key".into(), Value::String(row.get(2)?));
    map.insert("imported_at".into(), Value::String(row.get(3)?));
    let value = Value::Object(map);
    RecordImportKey::schema().validate(&value).map_err(|error| located(error, &record_id))?;
    Ok((record_id, value))
}

/// Say which stored record failed its published shape.
fn located(error: ValidationError, record_id: &str) -> Error {
    let detail = error.to_string();
    Error::Invalid(ValidationError::single(
        Vec::new(),
        format!("stored record {record_id} does not match its published shape: {detail}"),
    ))
}

fn history_error(message: String) -> Error {
    Error::Invalid(ValidationError::single(Vec::new(), message))
}

// ---- History checks -----------------------------------------------------------

/// What the history rules need to know about one current record.
struct RecordMeta {
    record_type: RecordType,
    current_revision: u64,
    is_deleted: bool,
    revisions_seen: u64,
    has_deletion: bool,
}

/// The complete-history rules of the package format, checked while streaming:
/// every revision through the current one, consecutive numbers, and a deletion
/// record exactly for deleted records, naming the current revision.
struct HistoryCheck {
    records: HashMap<String, RecordMeta>,
}

impl HistoryCheck {
    fn revision(&mut self, record_id: &str, record_type: RecordType, revision: u64) -> Result<()> {
        let Some(meta) = self.records.get_mut(record_id) else {
            return Err(history_error(format!("A retained revision needs its current record: {record_id}")));
        };
        if meta.record_type != record_type {
            return Err(history_error(format!("A revision must keep its record type: {record_id}")));
        }
        if revision != meta.revisions_seen + 1 {
            return Err(history_error(format!(
                "Record revision numbers must be complete and consecutive: {record_id}"
            )));
        }
        meta.revisions_seen = revision;
        Ok(())
    }

    fn deletion(&mut self, record_id: &str, record_revision: u64) -> Result<()> {
        let Some(meta) = self.records.get_mut(record_id) else {
            return Err(history_error(format!("A deletion record needs its retained current record: {record_id}")));
        };
        if meta.has_deletion {
            return Err(history_error(format!("A record can have only one current deletion record: {record_id}")));
        }
        if record_revision != meta.current_revision {
            return Err(history_error(format!(
                "A deletion record must name the current retained revision: {record_id}"
            )));
        }
        meta.has_deletion = true;
        Ok(())
    }

    fn import_key(&self, record_id: &str) -> Result<()> {
        if !self.records.contains_key(record_id) {
            return Err(history_error(format!("A provider import key needs its current record: {record_id}")));
        }
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        for (record_id, meta) in &self.records {
            if meta.revisions_seen != meta.current_revision {
                return Err(history_error(format!(
                    "A complete package needs every revision through current_revision: {record_id}"
                )));
            }
            if meta.is_deleted != meta.has_deletion {
                return Err(history_error(if meta.is_deleted {
                    format!("A deleted current record needs its deletion record: {record_id}")
                } else {
                    format!("A current deletion record requires is_deleted: {record_id}")
                }));
            }
        }
        Ok(())
    }
}

// ---- Measurement-series files -------------------------------------------------

struct SampleKey {
    record_id: String,
    recorded_at: String,
}

const SAMPLE_CHUNK_FROM: &str = "FROM measurement_samples
    JOIN records ON records.record_id = measurement_samples.record_id
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    WHERE measurement_samples.source_id = ?1
      AND measurement_samples.quantity = ?2
      AND (
        measurement_samples.recorded_at > ?3 OR
        (measurement_samples.recorded_at = ?3 AND records.record_id > ?4)
      )
    ORDER BY measurement_samples.recorded_at, records.record_id
    LIMIT ?5";

/// The samples of one chunk, from the first to the last in time order.
const SAMPLE_RANGE_WHERE: &str = "measurement_samples.source_id = ?1 AND
    measurement_samples.quantity = ?2 AND
    (measurement_samples.recorded_at > ?3 OR
      (measurement_samples.recorded_at = ?3 AND measurement_samples.record_id >= ?4)) AND
    (measurement_samples.recorded_at < ?5 OR
      (measurement_samples.recorded_at = ?5 AND measurement_samples.record_id <= ?6))";

fn write_series_files(
    snapshot: &Connection,
    staging_path: &Path,
    manifest_files: &mut Vec<ManifestFile>,
) -> Result<Vec<MeasurementSeriesFileReference>> {
    let groups: Vec<(String, String)> = snapshot
        .prepare(
            "SELECT source_id, quantity FROM measurement_samples
             GROUP BY source_id, quantity ORDER BY source_id, quantity",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    if !groups.is_empty() {
        files::create_private_dir(&staging_path.join("series"))?;
    }
    let mut references = Vec::new();
    for (source_id, quantity_text) in groups {
        let quantity = SeriesQuantity::parse(&quantity_text).ok_or_else(|| {
            Error::other(format!("measurement samples use an unsupported series quantity: {quantity_text}"))
        })?;
        let mut after_at = String::new();
        let mut after_id = String::new();
        let mut chunk_number = 0u32;
        loop {
            let mut metas: HashMap<String, RecordMeta> = HashMap::new();
            let mut keys: Vec<SampleKey> = Vec::new();
            {
                let mut statement = snapshot.prepare_cached(&format!(
                    "SELECT records.record_id, measurement_samples.recorded_at, records.current_revision, records.is_deleted
                     {SAMPLE_CHUNK_FROM}"
                ))?;
                let mut rows = statement.query(params![
                    source_id,
                    quantity_text,
                    after_at,
                    after_id,
                    SERIES_CHUNK_RECORDS as i64
                ])?;
                while let Some(row) = rows.next()? {
                    let record_id: String = row.get(0)?;
                    metas.insert(
                        record_id.clone(),
                        RecordMeta {
                            record_type: RecordType::MeasurementSample,
                            current_revision: row.get::<_, i64>(2)? as u64,
                            is_deleted: row.get::<_, i64>(3)? == 1,
                            revisions_seen: 0,
                            has_deletion: false,
                        },
                    );
                    keys.push(SampleKey { record_id, recorded_at: row.get(1)? });
                }
            }
            let (Some(first), Some(last)) = (keys.first(), keys.last()) else { break };
            chunk_number += 1;
            let relative = format!("series/{source_id}-{quantity_text}-{chunk_number:06}.ath-series.json");
            let mut history = HistoryCheck { records: metas };
            let range =
                params![source_id, quantity_text, first.recorded_at, first.record_id, last.recorded_at, last.record_id];

            let mut document = JsonDocument::create(&staging_path.join(&relative))?;
            document.scalar("athleticstandard_version", &Value::String(ATHLETIC_STANDARD_VERSION.into()))?;
            document.scalar("end_at", &Value::String(last.recorded_at.clone()))?;
            document.scalar("quantity", &Value::String(quantity_text.clone()))?;

            let mut array = document.array("record_deletions")?;
            {
                let mut statement = snapshot.prepare_cached(&format!(
                    "SELECT {DELETION_COLUMNS} FROM measurement_samples
                     JOIN record_deletions ON record_deletions.record_id = measurement_samples.record_id
                     WHERE {SAMPLE_RANGE_WHERE} ORDER BY record_deletions.record_id"
                ))?;
                let mut rows = statement.query(range)?;
                while let Some(row) = rows.next()? {
                    let (record_id, revision, value) = deletion_value(row)?;
                    history.deletion(&record_id, revision)?;
                    array.item(&value)?;
                }
            }
            let deletion_count = array.finish()?;

            let mut array = document.array("record_import_keys")?;
            {
                let mut statement = snapshot.prepare_cached(&format!(
                    "SELECT {IMPORT_KEY_COLUMNS} FROM measurement_samples
                     JOIN record_import_keys ON record_import_keys.record_id = measurement_samples.record_id
                     WHERE {SAMPLE_RANGE_WHERE}
                     ORDER BY record_import_keys.record_id, record_import_keys.match_key_method, record_import_keys.match_key"
                ))?;
                let mut rows = statement.query(range)?;
                while let Some(row) = rows.next()? {
                    let (record_id, value) = import_key_value(row)?;
                    history.import_key(&record_id)?;
                    array.item(&value)?;
                }
            }
            let import_key_count = array.finish()?;

            let mut array = document.array("record_revisions")?;
            {
                let mut statement = snapshot.prepare_cached(&format!(
                    "SELECT {REVISION_COLUMNS} FROM measurement_samples
                     JOIN record_revisions ON record_revisions.record_id = measurement_samples.record_id
                     WHERE {SAMPLE_RANGE_WHERE} ORDER BY record_revisions.record_id, record_revisions.revision"
                ))?;
                let mut rows = statement.query(range)?;
                while let Some(row) = rows.next()? {
                    let (record_id, record_type, revision, value) = revision_value(row)?;
                    history.revision(&record_id, record_type, revision)?;
                    array.item(&value)?;
                }
            }
            let revision_count = array.finish()?;

            let mut array = document.array("records")?;
            {
                let mut statement = snapshot.prepare_cached(&format!(
                    "SELECT {CURRENT_COLUMNS}, measurement_samples.recorded_at {SAMPLE_CHUNK_FROM}"
                ))?;
                let mut rows = statement.query(params![
                    source_id,
                    quantity_text,
                    after_at,
                    after_id,
                    SERIES_CHUNK_RECORDS as i64
                ])?;
                let mut index = 0;
                while let Some(row) = rows.next()? {
                    let (record_id, _, value) = current_value(row)?;
                    let typed_recorded_at: String = row.get(7)?;
                    let data = &value["data"];
                    if keys.get(index).map(|key| key.record_id.as_str()) != Some(record_id.as_str())
                        || data["recorded_at"].as_str() != Some(typed_recorded_at.as_str())
                        || data["source_id"].as_str() != Some(source_id.as_str())
                        || data["quantity"].as_str() != Some(quantity.as_str())
                    {
                        return Err(history_error(format!(
                            "measurement sample {record_id} does not match its typed series row"
                        )));
                    }
                    array.item(&value)?;
                    index += 1;
                }
            }
            let sample_count = array.finish()?;
            history.finish()?;

            document.scalar("source_id", &Value::String(source_id.clone()))?;
            document.scalar("start_at", &Value::String(first.recorded_at.clone()))?;
            let (sha256, byte_count) = document.finish()?;
            manifest_files.push(ManifestFile { file: relative.clone(), sha256: sha256.clone(), byte_count });
            references.push(MeasurementSeriesFileReference {
                file: relative,
                sha256,
                source_id: source_id.clone(),
                quantity,
                start_at: first.recorded_at.clone(),
                end_at: last.recorded_at.clone(),
                sample_count: sample_count as u64,
                revision_count: revision_count as u64,
                deletion_count: deletion_count as u64,
                import_key_count: import_key_count as u64,
            });
            after_at = last.recorded_at.clone();
            after_id = last.record_id.clone();
        }
    }
    Ok(references)
}

// ---- Attachments ---------------------------------------------------------------

fn write_attachments(
    snapshot: &Connection,
    source_directory: &Path,
    staging_path: &Path,
    manifest_files: &mut Vec<ManifestFile>,
) -> Result<()> {
    let mut statement = snapshot.prepare(&format!(
        "SELECT {CURRENT_COLUMNS}
         FROM records
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         WHERE records.record_type = 'attachment' AND records.is_deleted = 0
         ORDER BY records.record_id"
    ))?;
    let mut rows = statement.query([])?;
    let target = staging_path.join(ATTACHMENT_DIRECTORY);
    let mut created = false;
    while let Some(row) = rows.next()? {
        let (record_id, _, value) = current_value(row)?;
        let RecordData::Attachment(data) = RecordData::decode(RecordType::Attachment, &value["data"])? else {
            unreachable!("decode returns the requested type")
        };
        let name = stored_attachment_name(&record_id, &data);
        let source: PathBuf = source_directory.join(&name);
        let missing = || Error::not_found(format!("attachment file is missing: {} ({record_id})", source.display()));
        // A package copies only the attachment file itself: never one reached
        // through a symbolic link, which could name any file on the computer.
        match files::refuse_symbolic_link_components(&source, false) {
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
            other => other?,
        }
        if !fs::symlink_metadata(&source)?.is_file() {
            return Err(Error::refused(format!("attachment path is not a regular file: {}", source.display())));
        }
        let bytes = fs::read(&source).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => missing(),
            _ => error.into(),
        })?;
        check_attachment_bytes(&record_id, &data, &bytes)?;
        if bytes.len() as u64 != data.byte_count || durable::sha256_hex(&bytes) != data.sha256 {
            return Err(Error::refused(format!("attachment does not match its record: {record_id}")));
        }
        if !created {
            files::create_private_dir(&target)?;
            created = true;
        }
        let mut file = HashingFile::create(&target.join(&name))?;
        file.write_all(&bytes)?;
        file.finish()?;
        manifest_files.push(ManifestFile {
            file: format!("{ATTACHMENT_DIRECTORY}/{name}"),
            sha256: data.sha256.clone(),
            byte_count: bytes.len() as u64,
        });
    }
    Ok(())
}

// ---- Main document ---------------------------------------------------------------

const NON_SAMPLE_CURRENT_FROM: &str = "FROM records
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    WHERE records.record_type <> 'measurement_sample'";

fn write_main_document(
    snapshot: &Connection,
    staging_path: &Path,
    database_id: &str,
    database_revision: u64,
    series: &[MeasurementSeriesFileReference],
    manifest_files: &mut Vec<ManifestFile>,
) -> Result<u64> {
    let mut records: HashMap<String, RecordMeta> = HashMap::new();
    {
        let mut statement = snapshot.prepare(&format!(
            "SELECT records.record_id, records.record_type, records.current_revision, records.is_deleted
             {NON_SAMPLE_CURRENT_FROM}"
        ))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let record_id: String = row.get(0)?;
            let record_type_text: String = row.get(1)?;
            let record_type = RecordType::parse(&record_type_text).ok_or_else(|| {
                Error::other(format!("record {record_id} has unsupported record type {record_type_text}"))
            })?;
            records.insert(
                record_id,
                RecordMeta {
                    record_type,
                    current_revision: row.get::<_, i64>(2)? as u64,
                    is_deleted: row.get::<_, i64>(3)? == 1,
                    revisions_seen: 0,
                    has_deletion: false,
                },
            );
        }
    }
    let mut history = HistoryCheck { records };

    let mut document = JsonDocument::create(&staging_path.join(PACKAGE_DOCUMENT))?;
    document.scalar("athleticstandard_version", &Value::String(ATHLETIC_STANDARD_VERSION.into()))?;
    document.scalar("database_id", &Value::String(database_id.into()))?;
    document.scalar("database_revision", &Value::from(database_revision))?;

    let mut array = document.array("measurement_series_files")?;
    for reference in series {
        array.item(&json::to_value(reference))?;
    }
    array.finish()?;

    let mut array = document.array("record_deletions")?;
    {
        let mut statement = snapshot.prepare(&format!(
            "SELECT {DELETION_COLUMNS} FROM record_deletions
             JOIN records ON records.record_id = record_deletions.record_id
             WHERE records.record_type <> 'measurement_sample'
             ORDER BY record_deletions.record_id"
        ))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (record_id, revision, value) = deletion_value(row)?;
            history.deletion(&record_id, revision)?;
            array.item(&value)?;
        }
    }
    array.finish()?;

    let mut array = document.array("record_erasure_markers")?;
    {
        let mut statement = snapshot
            .prepare("SELECT record_id, erased_at, reason_code FROM record_erasure_markers ORDER BY record_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let record_id: String = row.get(0)?;
            let mut map = Map::new();
            map.insert("record_id".into(), Value::String(record_id.clone()));
            map.insert("erased_at".into(), Value::String(row.get(1)?));
            map.insert("reason_code".into(), Value::String(row.get(2)?));
            let value = Value::Object(map);
            RecordErasureMarker::schema().validate(&value).map_err(|error| located(error, &record_id))?;
            if history.records.contains_key(&record_id) {
                return Err(history_error(format!(
                    "An erased record cannot remain in current package records: {record_id}"
                )));
            }
            array.item(&value)?;
        }
    }
    let erasure_marker_count = array.finish()? as u64;

    let mut array = document.array("record_import_keys")?;
    {
        let mut statement = snapshot.prepare(&format!(
            "SELECT {IMPORT_KEY_COLUMNS} FROM record_import_keys
             JOIN records ON records.record_id = record_import_keys.record_id
             WHERE records.record_type <> 'measurement_sample'
             ORDER BY record_import_keys.record_id, record_import_keys.match_key_method, record_import_keys.match_key"
        ))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (record_id, value) = import_key_value(row)?;
            history.import_key(&record_id)?;
            array.item(&value)?;
        }
    }
    array.finish()?;

    let mut array = document.array("record_revisions")?;
    {
        let mut statement = snapshot.prepare(&format!(
            "SELECT {REVISION_COLUMNS} FROM record_revisions
             WHERE record_revisions.record_type <> 'measurement_sample'
             ORDER BY record_revisions.record_id, record_revisions.revision"
        ))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (record_id, record_type, revision, value) = revision_value(row)?;
            history.revision(&record_id, record_type, revision)?;
            array.item(&value)?;
        }
    }
    array.finish()?;

    let mut array = document.array("records")?;
    {
        let mut statement = snapshot
            .prepare(&format!("SELECT {CURRENT_COLUMNS} {NON_SAMPLE_CURRENT_FROM} ORDER BY records.record_id"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (_, _, value) = current_value(row)?;
            array.item(&value)?;
        }
    }
    array.finish()?;
    history.finish()?;

    let (sha256, byte_count) = document.finish()?;
    manifest_files.push(ManifestFile { file: PACKAGE_DOCUMENT.into(), sha256, byte_count });
    Ok(erasure_marker_count)
}

// ---- Counts ------------------------------------------------------------------------

fn count_map(connection: &Connection, sql: &str) -> Result<HashMap<String, u64>> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn record_counts(connection: &Connection) -> Result<Vec<ManifestRecordCount>> {
    let current = count_map(connection, "SELECT record_type, COUNT(*) FROM records GROUP BY record_type")?;
    let revisions = count_map(connection, "SELECT record_type, COUNT(*) FROM record_revisions GROUP BY record_type")?;
    let deletions = count_map(
        connection,
        "SELECT records.record_type, COUNT(*) FROM record_deletions
         JOIN records ON records.record_id = record_deletions.record_id GROUP BY records.record_type",
    )?;
    let import_keys = count_map(
        connection,
        "SELECT records.record_type, COUNT(*) FROM record_import_keys
         JOIN records ON records.record_id = record_import_keys.record_id GROUP BY records.record_type",
    )?;
    Ok(RecordType::ALL
        .iter()
        .map(|record_type| {
            let key = record_type.as_str();
            ManifestRecordCount {
                record_type: *record_type,
                current_record_count: current.get(key).copied().unwrap_or(0),
                revision_count: revisions.get(key).copied().unwrap_or(0),
                deletion_count: deletions.get(key).copied().unwrap_or(0),
                import_key_count: import_keys.get(key).copied().unwrap_or(0),
            }
        })
        .collect())
}
