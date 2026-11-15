//! Record writes: creation, correction, deletion, and provider import keys.
//!
//! A caller describes a complete change. The store checks every expectation,
//! validates every record, writes revisions and current state through the one
//! encoder, checks relationships, and advances the database revision once.
//! Callers cannot run these steps separately.

use rusqlite::{params, OptionalExtension, Transaction};

use super::encode::{self, Layout};
use super::Store;
use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::*;

/// A new record.
#[derive(Debug, Clone)]
pub struct NewRecord {
    pub data: RecordData,
    pub extensions: Option<Extensions>,
    pub record_id: Option<String>,
    pub created_at: Option<String>,
    pub became_known_at: Option<String>,
    pub became_known_at_basis: BecameKnownAtBasis,
    pub changed_by: RecordActor,
}

impl NewRecord {
    pub fn new(data: RecordData, changed_by: RecordActor) -> Self {
        NewRecord {
            data,
            extensions: None,
            record_id: None,
            created_at: None,
            became_known_at: None,
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            changed_by,
        }
    }
}

/// What a correction does with existing extensions.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionsUpdate {
    /// Keep the current revision's extensions.
    Keep,
    /// Replace them; `None` removes them.
    Set(Option<Extensions>),
}

/// How a stale correction is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleStyle {
    /// A direct correction names the current and expected revisions.
    Revision,
    /// A previewed batch says the record changed after the preview.
    Preview,
}

/// A new revision of an existing record.
#[derive(Debug, Clone)]
pub struct Correction {
    pub record_id: String,
    pub expected_revision: u64,
    pub data: RecordData,
    pub extensions: ExtensionsUpdate,
    pub changed_by: RecordActor,
    pub became_known_at: Option<String>,
    pub became_known_at_basis: BecameKnownAtBasis,
    pub stale_style: StaleStyle,
}

/// A normal deletion: the record and history stay, hidden from active views.
#[derive(Debug, Clone)]
pub struct Deletion {
    pub record_id: String,
    pub expected_revision: u64,
    pub deleted_by: RecordActor,
    pub deleted_at: Option<String>,
}

/// One complete logical change.
#[derive(Debug, Clone, Default)]
pub struct Changes {
    /// Refuse when the database revision is not this value.
    pub expected_database_revision: Option<u64>,
    /// Refuse when any of these records is not at this revision or is deleted.
    pub expected_current_revisions: Vec<(String, u64)>,
    pub creates: Vec<NewRecord>,
    pub corrections: Vec<Correction>,
    pub deletions: Vec<Deletion>,
    pub import_keys: Vec<RecordImportKey>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.creates.is_empty()
            && self.corrections.is_empty()
            && self.deletions.is_empty()
            && self.import_keys.is_empty()
    }
}

/// The result of a write.
#[derive(Debug, Clone)]
pub struct Applied {
    /// Created and corrected records, in the order they were written.
    pub records: Vec<CurrentRecord>,
    pub database_revision: u64,
}

impl Store {
    /// Apply a complete change in one transaction.
    pub fn apply(&mut self, changes: Changes) -> Result<Applied> {
        if changes.is_empty() {
            return Err(Error::other("A write needs at least one change"));
        }
        let layout = self.layout();
        self.transaction(|tx| apply_in(tx, layout, changes))
    }
}

/// Apply a change inside an existing transaction. Package restore and imports
/// call this when they own a larger transaction.
pub(crate) fn apply_in(tx: &Transaction<'_>, layout: Layout, changes: Changes) -> Result<Applied> {
    let records = write_in(tx, layout, changes)?;
    let database_revision = advance_database_revision(tx)?;
    Ok(Applied { records, database_revision })
}

/// Advance the database revision once for a complete logical change.
pub(crate) fn advance_database_revision(tx: &Transaction<'_>) -> Result<u64> {
    tx.execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])?;
    super::database_revision(tx)
}

/// Check and write one part of a change without advancing the database
/// revision. A provider import writes a large plan in bounded batches inside
/// one transaction, then calls `advance_database_revision` once. Callers
/// outside this crate use `apply_in`, which always does both.
pub(crate) fn write_in(tx: &Transaction<'_>, layout: Layout, changes: Changes) -> Result<Vec<CurrentRecord>> {
    if let Some(expected) = changes.expected_database_revision {
        if super::database_revision(tx)? != expected {
            return Err(Error::stale("Database changed after provider import preview; preview it again"));
        }
    }
    for (record_id, revision) in &changes.expected_current_revisions {
        let row: Option<(i64, i64)> = tx
            .query_row("SELECT current_revision, is_deleted FROM records WHERE record_id = ?1", [record_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        match row {
            None => return Err(Error::stale(format!("Expected record does not exist: {record_id}"))),
            Some((current, deleted)) if current as u64 != *revision || deleted != 0 => {
                return Err(Error::stale(format!("Record {record_id} changed after preview; preview it again")));
            }
            _ => {}
        }
    }

    let now = time::now_utc();
    let mut written = Vec::new();
    // Attachments first: other records in the same change may link to them.
    let (attachments, others): (Vec<NewRecord>, Vec<NewRecord>) =
        changes.creates.into_iter().partition(|create| create.data.record_type() == RecordType::Attachment);
    for create in attachments {
        written.push(insert_record(tx, layout, create, &now)?);
    }
    for correction in changes.corrections {
        written.push(correct_record(tx, layout, correction, &now)?);
    }
    for create in others {
        written.push(insert_record(tx, layout, create, &now)?);
    }
    for deletion in changes.deletions {
        delete_record(tx, deletion, &now)?;
    }
    for key in changes.import_keys {
        insert_import_key(tx, &key)?;
    }
    for record in &written {
        validate_relationships(tx, &record.data)?;
    }
    Ok(written)
}

fn checked_timestamp(value: Option<String>, fallback: &str, name: &str) -> Result<String> {
    let value = value.unwrap_or_else(|| fallback.to_string());
    if !time::is_utc_timestamp(&value) {
        return Err(Error::Invalid(ValidationError::single(
            vec![PathSegment::Key(name.into())],
            "Invalid ISO datetime",
        )));
    }
    Ok(value)
}

/// Validate a complete current record exactly as a package reader would.
fn checked_record(record: CurrentRecord) -> Result<CurrentRecord> {
    Ok(CurrentRecord::from_value(&record.to_value())?)
}

#[allow(clippy::too_many_arguments)]
fn insert_revision(
    tx: &Transaction<'_>,
    record_id: &str,
    revision: u64,
    record_type: RecordType,
    record_json: &str,
    extensions: Option<&Extensions>,
    became_known_at: &str,
    basis: BecameKnownAtBasis,
    changed_by: &RecordActor,
) -> Result<()> {
    let extensions_json = extensions.map(|map| json::to_string(&Value::Object(map.clone())));
    tx.prepare_cached(
        "INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        record_id,
        revision as i64,
        record_type.as_str(),
        record_json,
        extensions_json,
        became_known_at,
        basis.as_str(),
        json::encode(changed_by),
        if revision == 1 { None } else { Some(revision as i64 - 1) },
    ])?;
    Ok(())
}

fn insert_record(tx: &Transaction<'_>, layout: Layout, create: NewRecord, now: &str) -> Result<CurrentRecord> {
    let record_id = create.record_id.unwrap_or_else(crate::ids::new_record_id);
    let created_at = checked_timestamp(create.created_at, now, "created_at")?;
    let became_known_at = checked_timestamp(create.became_known_at, &created_at, "became_known_at")?;
    let record = checked_record(CurrentRecord {
        record_id,
        created_at,
        current_revision: 1,
        is_deleted: false,
        extensions: create.extensions,
        data: create.data,
    })?;
    let record_type = record.record_type();
    let record_json = record.data.to_json();
    tx.prepare_cached(
        "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, ?2, 1, ?3, 0)",
    )?
    .execute(params![record.record_id, record_type.as_str(), record.created_at])?;
    insert_revision(
        tx,
        &record.record_id,
        1,
        record_type,
        &record_json,
        record.extensions.as_ref(),
        &became_known_at,
        create.became_known_at_basis,
        &create.changed_by,
    )?;
    encode::write_current_state(tx, layout, &record.record_id, &record.data, &record_json)?;
    Ok(record)
}

struct CurrentRow {
    record_type: String,
    current_revision: u64,
    created_at: String,
    is_deleted: bool,
    became_known_at: String,
    extensions_json: Option<String>,
}

fn current_row(tx: &Transaction<'_>, record_id: &str) -> Result<CurrentRow> {
    tx.query_row(
        "SELECT records.record_type, records.current_revision, records.created_at, records.is_deleted,
                record_revisions.became_known_at, record_revisions.extensions_json
         FROM records
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         WHERE records.record_id = ?1",
        [record_id],
        |row| {
            Ok(CurrentRow {
                record_type: row.get(0)?,
                current_revision: row.get::<_, i64>(1)? as u64,
                created_at: row.get(2)?,
                is_deleted: row.get::<_, i64>(3)? != 0,
                became_known_at: row.get(4)?,
                extensions_json: row.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Error::not_found(format!("Record not found: {record_id}")))
}

fn correct_record(tx: &Transaction<'_>, layout: Layout, correction: Correction, now: &str) -> Result<CurrentRecord> {
    let record_id = correction.record_id;
    let current = current_row(tx, &record_id)?;
    match correction.stale_style {
        StaleStyle::Preview => {
            if current.current_revision != correction.expected_revision || current.is_deleted {
                return Err(Error::stale(format!("Record {record_id} changed after preview; preview it again")));
            }
        }
        StaleStyle::Revision => {
            if current.current_revision != correction.expected_revision {
                return Err(Error::stale(format!(
                    "Record {record_id} is at revision {}, not expected revision {}",
                    current.current_revision, correction.expected_revision
                )));
            }
            if current.is_deleted {
                return Err(Error::refused(format!("Deleted record cannot be corrected: {record_id}")));
            }
        }
    }
    if current.record_type != correction.data.record_type().as_str() {
        return Err(Error::refused(format!(
            "Record {record_id} is a {}, not a {}",
            current.record_type,
            correction.data.record_type()
        )));
    }
    let became_known_at = checked_timestamp(correction.became_known_at, now, "became_known_at")?;
    if time::parse_utc_timestamp(&became_known_at) < time::parse_utc_timestamp(&current.became_known_at) {
        return Err(Error::refused(format!(
            "Revision knowledge time cannot precede revision {}",
            current.current_revision
        )));
    }
    let extensions = match correction.extensions {
        ExtensionsUpdate::Keep => match &current.extensions_json {
            Some(text) => match json::parse(text)? {
                Value::Object(map) => Some(map),
                Value::Null => None,
                _ => return Err(Error::other("extensions_json must be an object")),
            },
            None => None,
        },
        ExtensionsUpdate::Set(extensions) => extensions,
    };
    let next = current.current_revision + 1;
    let record = checked_record(CurrentRecord {
        record_id: record_id.clone(),
        created_at: current.created_at,
        current_revision: next,
        is_deleted: false,
        extensions,
        data: correction.data,
    })?;
    let record_json = record.data.to_json();
    insert_revision(
        tx,
        &record_id,
        next,
        record.record_type(),
        &record_json,
        record.extensions.as_ref(),
        &became_known_at,
        correction.became_known_at_basis,
        &correction.changed_by,
    )?;
    encode::write_current_state(tx, layout, &record_id, &record.data, &record_json)?;
    tx.prepare_cached("UPDATE records SET current_revision = ?1 WHERE record_id = ?2")?
        .execute(params![next as i64, record_id])?;
    Ok(record)
}

fn delete_record(tx: &Transaction<'_>, deletion: Deletion, now: &str) -> Result<()> {
    let record_id = deletion.record_id;
    let current = current_row(tx, &record_id)?;
    if current.current_revision != deletion.expected_revision {
        return Err(Error::stale(format!(
            "Record {record_id} is at revision {}, not expected revision {}",
            current.current_revision, deletion.expected_revision
        )));
    }
    if current.is_deleted {
        return Err(Error::refused(format!("Record is already deleted: {record_id}")));
    }
    let deleted_at = checked_timestamp(deletion.deleted_at, now, "deleted_at")?;
    if time::parse_utc_timestamp(&deleted_at) < time::parse_utc_timestamp(&current.became_known_at) {
        return Err(Error::refused("Deletion time cannot precede the current revision knowledge time"));
    }
    tx.execute(
        "INSERT INTO record_deletions (record_id, record_revision, deleted_at, deleted_by_json) VALUES (?1, ?2, ?3, ?4)",
        params![record_id, current.current_revision as i64, deleted_at, json::encode(&deletion.deleted_by)],
    )?;
    tx.execute("UPDATE records SET is_deleted = 1 WHERE record_id = ?1", [&record_id])?;
    Ok(())
}

/// Insert a provider import key. A key that already names the same record is
/// kept. A key that names another record means the database changed after a
/// preview.
pub(crate) fn insert_import_key(tx: &Transaction<'_>, key: &RecordImportKey) -> Result<()> {
    let existing: Option<String> = tx
        .prepare_cached("SELECT record_id FROM record_import_keys WHERE match_key_method = ?1 AND match_key = ?2")?
        .query_row(params![key.match_key_method, key.match_key], |row| row.get(0))
        .optional()?;
    match existing {
        Some(owner) if owner == key.record_id => Ok(()),
        Some(_) => Err(Error::stale("Provider import key changed after preview")),
        None => {
            tx.prepare_cached(
                "INSERT INTO record_import_keys (record_id, match_key_method, match_key, imported_at) VALUES (?1, ?2, ?3, ?4)",
            )?
            .execute(params![key.record_id, key.match_key_method, key.match_key, key.imported_at])?;
            Ok(())
        }
    }
}

/// Read one retained revision's data, requiring its record type.
pub(crate) fn revision_data(
    tx: &rusqlite::Connection,
    reference: &EvidenceReference,
    expected: RecordType,
) -> Result<RecordData> {
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT record_type, record_json FROM record_revisions WHERE record_id = ?1 AND revision = ?2",
            params![reference.record_id, reference.revision as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((record_type, record_json)) = row else {
        return Err(Error::not_found(format!(
            "Referenced record revision does not exist: {}@{}",
            reference.record_id, reference.revision
        )));
    };
    if record_type != expected.as_str() {
        return Err(Error::refused(format!(
            "Referenced {}@{} is {record_type}, not {expected}",
            reference.record_id, reference.revision
        )));
    }
    Ok(RecordData::decode(expected, &json::parse(&record_json)?)?)
}

fn linked(tx: &rusqlite::Connection, link: &PredictionResultLinkData) -> Result<(PredictionData, BenchmarkResultData)> {
    let RecordData::Prediction(prediction) = revision_data(tx, &link.prediction, RecordType::Prediction)? else {
        unreachable!("revision_data checks the record type")
    };
    let RecordData::BenchmarkResult(result) = revision_data(tx, &link.benchmark_result, RecordType::BenchmarkResult)?
    else {
        unreachable!("revision_data checks the record type")
    };
    if prediction.benchmark_definition_id != result.benchmark_definition_id {
        return Err(Error::refused("Prediction and benchmark result use different benchmark definitions"));
    }
    Ok((prediction, result))
}

/// Relationship rules SQLite cannot check: a link joins a prediction and a
/// result for the same benchmark, and a saved grade's arithmetic matches the
/// exact revisions it names.
pub(crate) fn validate_relationships(tx: &rusqlite::Connection, data: &RecordData) -> Result<()> {
    match data {
        RecordData::PredictionResultLink(link) => {
            linked(tx, link)?;
            Ok(())
        }
        RecordData::PredictionGrade(grade) => {
            let RecordData::PredictionResultLink(link) =
                revision_data(tx, &grade.prediction_result_link, RecordType::PredictionResultLink)?
            else {
                unreachable!("revision_data checks the record type")
            };
            let (prediction, result) = linked(tx, &link)?;
            if json::encode(&grade.predicted_score) != json::encode(&prediction.predicted_score) {
                return Err(Error::refused(
                    "Saved grade predicted score does not match the linked prediction revision",
                ));
            }
            if json::encode(&grade.actual_score) != json::encode(&result.score) {
                return Err(Error::refused("Saved grade actual score does not match the linked result revision"));
            }
            let score_type: Option<String> = tx
                .query_row(
                    "SELECT score_type FROM benchmark_definitions WHERE record_id = ?1",
                    [&prediction.benchmark_definition_id],
                    |row| row.get(0),
                )
                .optional()?;
            let score_type = score_type.ok_or_else(|| Error::refused("Linked benchmark definition does not exist"))?;
            let expected = crate::model::grade::grade_arithmetic(&prediction, &result.score, &score_type)
                .map_err(Error::Refused)?;
            if grade.signed_error != expected.signed_error
                || grade.absolute_error_percentage != expected.absolute_error_percentage
                || grade.result_in_predicted_range != expected.result_in_predicted_range
            {
                return Err(Error::refused("Saved grade arithmetic does not match its linked revisions"));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
