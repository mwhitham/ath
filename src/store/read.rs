//! Record reads.
//!
//! Reads decode stored JSON and validate it. A stored record that no longer
//! matches its shape is reported, not silently repaired.

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::Store;
use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::*;

/// Options for listing current records.
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub record_type: Option<RecordType>,
    pub include_deleted: bool,
    /// Between 1 and 10,000.
    pub limit: u32,
}

impl Default for ListOptions {
    fn default() -> Self {
        ListOptions { record_type: None, include_deleted: false, limit: 100 }
    }
}

pub const MAX_LIST_LIMIT: u32 = 10_000;

const CURRENT_SELECT: &str = "SELECT records.record_id, records.record_type, records.current_revision,
        records.created_at, records.is_deleted, record_revisions.record_json, record_revisions.extensions_json
   FROM records
   JOIN record_revisions
     ON record_revisions.record_id = records.record_id
    AND record_revisions.revision = records.current_revision";

fn parse_extensions(text: Option<String>) -> Result<Option<Extensions>> {
    match text {
        None => Ok(None),
        Some(text) => match json::parse(&text)? {
            Value::Null => Ok(None),
            value @ Value::Object(_) => {
                crate::model::extensions::record_extensions().validate(&value)?;
                match value {
                    Value::Object(map) => Ok(Some(map)),
                    _ => unreachable!(),
                }
            }
            _ => Err(Error::other("extensions_json must be an object")),
        },
    }
}

/// Build a current record from a row of `CURRENT_SELECT`.
pub fn current_from_row(row: &Row<'_>) -> Result<CurrentRecord> {
    let record_id: String = row.get(0)?;
    let record_type: String = row.get(1)?;
    let current_revision: i64 = row.get(2)?;
    let created_at: String = row.get(3)?;
    let is_deleted: i64 = row.get(4)?;
    let record_json: String = row.get(5)?;
    let extensions_json: Option<String> = row.get(6)?;
    let mut value = serde_json::Map::new();
    value.insert("record_id".into(), Value::String(record_id));
    value.insert("created_at".into(), Value::String(created_at));
    value.insert("current_revision".into(), Value::from(current_revision));
    value.insert("is_deleted".into(), Value::Bool(is_deleted == 1));
    if let Some(extensions) = parse_extensions(extensions_json)? {
        value.insert("extensions".into(), Value::Object(extensions));
    }
    value.insert("record_type".into(), Value::String(record_type));
    value.insert("data".into(), json::parse(&record_json)?);
    Ok(CurrentRecord::from_value(&Value::Object(value))?)
}

/// Build a revision from `record_revisions` columns.
pub fn revision_from_row(row: &Row<'_>) -> Result<RecordRevision> {
    let record_id: String = row.get("record_id")?;
    let record_type: String = row.get("record_type")?;
    let revision: i64 = row.get("revision")?;
    let previous: Option<i64> = row.get("previous_revision")?;
    let became_known_at: String = row.get("became_known_at")?;
    let basis: String = row.get("became_known_at_basis")?;
    let changed_by: String = row.get("changed_by_json")?;
    let record_json: String = row.get("record_json")?;
    let extensions_json: Option<String> = row.get("extensions_json")?;
    let mut value = serde_json::Map::new();
    value.insert("record_id".into(), Value::String(record_id));
    value.insert("revision".into(), Value::from(revision));
    value.insert("previous_revision".into(), previous.map_or(Value::Null, Value::from));
    value.insert("became_known_at".into(), Value::String(became_known_at));
    value.insert("became_known_at_basis".into(), Value::String(basis));
    value.insert("changed_by".into(), json::parse(&changed_by)?);
    value.insert("record_type".into(), Value::String(record_type));
    value.insert("data".into(), json::parse(&record_json)?);
    if let Some(extensions) = parse_extensions(extensions_json)? {
        value.insert("extensions".into(), Value::Object(extensions));
    }
    Ok(RecordRevision::from_value(&Value::Object(value))?)
}

pub const REVISION_COLUMNS: &str = "record_id, record_type, revision, previous_revision, became_known_at,
    became_known_at_basis, changed_by_json, record_json, extensions_json";

/// One current record, including a deleted one.
pub(crate) fn get_record(connection: &Connection, record_id: &str) -> Result<Option<CurrentRecord>> {
    let mut statement = connection.prepare_cached(&format!("{CURRENT_SELECT} WHERE records.record_id = ?1"))?;
    let mut rows = statement.query([record_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(current_from_row(row)?)),
        None => Ok(None),
    }
}

/// Current records in creation order.
pub(crate) fn list_records(connection: &Connection, options: &ListOptions) -> Result<Vec<CurrentRecord>> {
    if options.limit < 1 || options.limit > MAX_LIST_LIMIT {
        return Err(Error::other("Record list limit must be between 1 and 10000"));
    }
    let mut conditions = Vec::new();
    let mut args: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(record_type) = options.record_type {
        conditions.push("records.record_type = ?");
        args.push(rusqlite::types::Value::Text(record_type.as_str().into()));
    }
    if !options.include_deleted {
        conditions.push("records.is_deleted = 0");
    }
    args.push(rusqlite::types::Value::Integer(i64::from(options.limit)));
    let filter = if conditions.is_empty() { String::new() } else { format!("WHERE {}", conditions.join(" AND ")) };
    let sql = format!("{CURRENT_SELECT} {filter} ORDER BY records.created_at, records.record_id LIMIT ?");
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(args))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(current_from_row(row)?);
    }
    Ok(out)
}

/// Every retained revision of a record, oldest first.
pub(crate) fn revisions(connection: &Connection, record_id: &str) -> Result<Vec<RecordRevision>> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} FROM record_revisions WHERE record_id = ?1 ORDER BY revision"
    ))?;
    let mut rows = statement.query([record_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(revision_from_row(row)?);
    }
    Ok(out)
}

/// One retained revision.
pub(crate) fn revision(connection: &Connection, record_id: &str, revision: u64) -> Result<Option<RecordRevision>> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {REVISION_COLUMNS} FROM record_revisions WHERE record_id = ?1 AND revision = ?2"
    ))?;
    let mut rows = statement.query(params![record_id, revision as i64])?;
    match rows.next()? {
        Some(row) => Ok(Some(revision_from_row(row)?)),
        None => Ok(None),
    }
}

/// The current revision number and deletion state, without decoding data.
pub(crate) fn current_revision(connection: &Connection, record_id: &str) -> Result<Option<(u64, bool)>> {
    Ok(connection
        .prepare_cached("SELECT current_revision, is_deleted FROM records WHERE record_id = ?1")?
        .query_row([record_id], |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? != 0)))
        .optional()?)
}

impl Store {
    pub fn get_record(&self, record_id: &str) -> Result<Option<CurrentRecord>> {
        get_record(self.connection(), record_id)
    }

    pub fn list_records(&self, options: &ListOptions) -> Result<Vec<CurrentRecord>> {
        list_records(self.connection(), options)
    }

    pub fn revisions(&self, record_id: &str) -> Result<Vec<RecordRevision>> {
        revisions(self.connection(), record_id)
    }

    pub fn revision(&self, record_id: &str, number: u64) -> Result<Option<RecordRevision>> {
        revision(self.connection(), record_id, number)
    }
}
