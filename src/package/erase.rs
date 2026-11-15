//! Privacy erasure.
//!
//! Erasure is different from normal deletion. It removes a record, every
//! record that depends on it, all of their retained revisions, and their
//! attachment files. Each erased record leaves only a value-free marker: its
//! ID, the time, and a reason code.
//!
//! The database change and the file removals cannot commit together. The
//! erasure therefore commits a pending removal for each attachment file in
//! the same transaction that erases the records. It reports success only
//! after every file is removed and the folder is synchronized. A removal that
//! fails stays pending; `ath check` reports it and the next writable command
//! retries it.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;

use super::attachments::{
    attachment_directory, relative_attachment_path, stored_attachment_name, ATTACHMENT_DIRECTORY,
};
use super::failpoint;
use crate::error::{Error, Result};
use crate::files::{self, durable, process};
use crate::json::Value;
use crate::model::*;
use crate::store::{self, read, Store};

/// One record the erasure removes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErasureRecord {
    pub record_id: String,
    pub record_type: RecordType,
    pub current_revision: u64,
}

/// One attachment file the erasure removes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErasureAttachment {
    pub record_id: String,
    pub path: String,
    /// True when the file exists now.
    pub present: bool,
}

/// Everything one erasure would remove, built against a known database revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErasurePreview {
    pub root_record_id: String,
    pub root_expected_revision: u64,
    pub database_revision: u64,
    pub records: Vec<ErasureRecord>,
    pub attachments: Vec<ErasureAttachment>,
}

/// A completed erasure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErasureResult {
    pub erased_record_ids: Vec<String>,
    pub erased_attachment_count: usize,
    pub database_revision: u64,
}

/// Every record that depends on the root, by any stored reference, and the
/// root itself. Attachment links are followed in both directions.
const DEPENDENCY_SQL: &str = "
    WITH RECURSIVE
    dependency_edges(parent_id, dependent_id) AS (
      SELECT source_id, record_id FROM source_devices
      UNION ALL SELECT source_id, record_id FROM point_measurements
      UNION ALL SELECT source_id, record_id FROM measurement_samples
      UNION ALL SELECT source_id, record_id FROM sleep_sessions
      UNION ALL SELECT source_id, record_id FROM workout_sessions
      UNION ALL SELECT source_id, record_id FROM benchmark_results
      UNION ALL SELECT source_id, record_id FROM provider_summaries
      UNION ALL SELECT source_id, record_id FROM provider_scores
      UNION ALL SELECT benchmark_definition_id, record_id FROM benchmark_results
      UNION ALL SELECT workout_session_id, record_id FROM benchmark_results
        WHERE workout_session_id IS NOT NULL
      UNION ALL SELECT benchmark_definition_id, record_id FROM predictions
      UNION ALL SELECT prediction_id, record_id FROM prediction_result_links
      UNION ALL SELECT benchmark_result_id, record_id FROM prediction_result_links
      UNION ALL SELECT prediction_result_link_id, record_id FROM prediction_grades
      UNION ALL SELECT prediction_grade_id, record_id FROM prediction_error_analyses
      UNION ALL SELECT training_plan_id, record_id FROM planned_sessions
        WHERE training_plan_id IS NOT NULL
      UNION ALL SELECT completed_workout_session_id, record_id FROM planned_sessions
        WHERE completed_workout_session_id IS NOT NULL
      UNION ALL SELECT target_benchmark_definition_id, record_id FROM goals
        WHERE target_benchmark_definition_id IS NOT NULL
      UNION ALL SELECT nutrition_entry_id, record_id FROM nutrition_model_interpretations
      UNION ALL SELECT evidence_record_id, point_measurement_id
        FROM point_measurement_calculation_inputs
      UNION ALL SELECT evidence_record_id, authored_record_id
        FROM authored_record_evidence
      UNION ALL SELECT evidence_record_id, prediction_error_analysis_id
        FROM prediction_error_causes
      UNION ALL SELECT owner_record_id, attachment_record_id
        FROM record_attachment_links
      UNION ALL SELECT attachment_record_id, owner_record_id
        FROM record_attachment_links
    ),
    dependencies(record_id) AS (
      VALUES (?1)
      UNION
      SELECT dependency_edges.dependent_id
      FROM dependency_edges
      JOIN dependencies
        ON dependencies.record_id = dependency_edges.parent_id
    )
    SELECT
      records.record_id,
      records.record_type,
      records.current_revision,
      records.created_at,
      records.is_deleted,
      record_revisions.record_json,
      record_revisions.extensions_json
    FROM dependencies
    JOIN records ON records.record_id = dependencies.record_id
    JOIN record_revisions
      ON record_revisions.record_id = records.record_id
     AND record_revisions.revision = records.current_revision
    ORDER BY records.created_at, records.record_id";

fn dependent_records(connection: &Connection, root: &str) -> Result<Vec<CurrentRecord>> {
    let mut statement = connection.prepare(DEPENDENCY_SQL)?;
    let mut rows = statement.query([root])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        records.push(read::current_from_row(row)?);
    }
    Ok(records)
}

fn checked_record_id(record_id: &str) -> Result<()> {
    record_id_schema().validate(&Value::String(record_id.to_string()))?;
    Ok(())
}

fn record_id_schema() -> crate::model::schema::SchemaRef {
    crate::model::common::record_id()
}

fn attachment_name(record: &CurrentRecord) -> Option<String> {
    match &record.data {
        RecordData::Attachment(data) => Some(stored_attachment_name(&record.record_id, data)),
        _ => None,
    }
}

/// List everything an erasure of `record_id` would remove. Refuses a record
/// that is missing, already erased, or not at `expected_revision`.
pub fn preview_erasure(store: &Store, record_id: &str, expected_revision: u64) -> Result<ErasurePreview> {
    checked_record_id(record_id)?;
    if expected_revision < 1 {
        return Err(Error::other("Expected revision must be a positive whole number"));
    }
    let connection = store.connection();
    let records = dependent_records(connection, record_id)?;
    let Some(root) = records.iter().find(|record| record.record_id == record_id) else {
        let erased =
            connection.prepare("SELECT 1 FROM record_erasure_markers WHERE record_id = ?1")?.exists([record_id])?;
        return Err(Error::not_found(if erased {
            format!("Record was already erased: {record_id}")
        } else {
            format!("Record not found: {record_id}")
        }));
    };
    if root.current_revision != expected_revision {
        return Err(Error::stale(format!(
            "Record {record_id} is at revision {}, not expected revision {expected_revision}",
            root.current_revision
        )));
    }
    let directory = attachment_directory(store.path());
    let mut attachments = Vec::new();
    for record in &records {
        let Some(name) = attachment_name(record) else { continue };
        let path = directory.join(name);
        files::refuse_symbolic_link_components(&path, true)?;
        let present = match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(Error::refused(format!("Attachment is not a regular file: {}", path.display())));
            }
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        attachments.push(ErasureAttachment {
            record_id: record.record_id.clone(),
            path: path.display().to_string(),
            present,
        });
    }
    Ok(ErasurePreview {
        root_record_id: record_id.to_string(),
        root_expected_revision: expected_revision,
        database_revision: store.database_revision()?,
        records: records
            .iter()
            .map(|record| ErasureRecord {
                record_id: record.record_id.clone(),
                record_type: record.record_type(),
                current_revision: record.current_revision,
            })
            .collect(),
        attachments,
    })
}

/// Erase what `preview` lists. The database must not have changed since the
/// preview. Returns an error when an attachment file could not be removed;
/// the records are then erased, and the removal stays pending.
pub fn erase_record(
    store: &mut Store,
    preview: &ErasurePreview,
    reason_code: ErasureReasonCode,
    erased_at: Option<&str>,
) -> Result<ErasureResult> {
    let erased_at = match erased_at {
        Some(text) => {
            crate::model::common::utc_timestamp().validate(&Value::String(text.to_string()))?;
            text.to_string()
        }
        None => time::now_utc(),
    };
    let operation_id = format!("{}-{}", process::current_pid(), uuid::Uuid::now_v7().simple());
    let directory = attachment_directory(store.path());
    let expected_ids: Vec<String> = preview.records.iter().map(|record| record.record_id.clone()).collect();

    let (database_revision, names) = store.transaction(|tx| {
        if store::database_revision(tx)? != preview.database_revision {
            return Err(Error::stale("Database changed after the erasure preview; preview it again"));
        }
        if read::current_revision(tx, &preview.root_record_id)?.map(|(revision, _)| revision)
            != Some(preview.root_expected_revision)
        {
            return Err(Error::stale("Record changed after the erasure preview; preview it again"));
        }
        let fresh = dependent_records(tx, &preview.root_record_id)?;
        let fresh_ids: Vec<&str> = fresh.iter().map(|record| record.record_id.as_str()).collect();
        if fresh_ids != expected_ids.iter().map(String::as_str).collect::<Vec<_>>() {
            return Err(Error::stale("Erasure dependencies changed after preview; preview it again"));
        }
        let names: Vec<String> = fresh.iter().filter_map(attachment_name).collect();
        remove_records(tx, &expected_ids, &erased_at, reason_code)?;
        let now = time::now_utc();
        let mut insert = tx.prepare_cached(
            "INSERT INTO internal_pending_file_operations (operation_id, operation_kind, relative_path, created_at)
             VALUES (?1, 'attachment_removal', ?2, ?3)",
        )?;
        for name in &names {
            insert.execute(params![operation_id, relative_attachment_path(name), now])?;
        }
        tx.execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])?;
        failpoint!("erase.before_commit");
        Ok((store::database_revision(tx)?, names))
    })?;
    failpoint!("erase.after_commit");

    let (removed, failures) = remove_attachment_files(&directory, &names);
    if failures.is_empty() {
        failpoint!("erase.after_file_removal");
        store.transaction(|tx| {
            tx.execute("DELETE FROM internal_pending_file_operations WHERE operation_id = ?1", [&operation_id])?;
            Ok(())
        })?;
        return Ok(ErasureResult {
            erased_record_ids: expected_ids,
            erased_attachment_count: removed,
            database_revision,
        });
    }
    Err(Error::other(format!(
        "erased {} record{} at database revision {database_revision}, but {} attachment file{} could not be removed ({}). \
         The removal stays pending: ath check reports it, and the next writable ath command retries it.",
        expected_ids.len(),
        if expected_ids.len() == 1 { "" } else { "s" },
        failures.len(),
        if failures.len() == 1 { "" } else { "s" },
        failures.join("; ")
    )))
}

/// Remove files by stored name. Returns how many existed and were removed,
/// and one message per failure.
fn remove_attachment_files(directory: &Path, names: &[String]) -> (usize, Vec<String>) {
    let mut removed = 0;
    let mut failures = Vec::new();
    for name in names {
        let path: PathBuf = directory.join(name);
        let outcome = (|| -> Result<bool> {
            failpoint!("erase.remove_file");
            files::refuse_symbolic_link_components(&path, true)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if !metadata.is_file() => {
                    Err(Error::refused(format!("Attachment is not a regular file: {}", path.display())))
                }
                Ok(_) => Ok(durable::remove_file_if_present(&path)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(error.into()),
            }
        })();
        match outcome {
            Ok(true) => removed += 1,
            Ok(false) => {}
            Err(error) => failures.push(format!("{ATTACHMENT_DIRECTORY}/{name}: {error}")),
        }
    }
    if removed > 0 && directory.is_dir() {
        if let Err(error) = files::sync_directory(directory) {
            failures.push(format!("{ATTACHMENT_DIRECTORY}: {error}"));
        }
    }
    (removed, failures)
}

fn remove_records(
    tx: &rusqlite::Transaction<'_>,
    record_ids: &[String],
    erased_at: &str,
    reason_code: ErasureReasonCode,
) -> Result<()> {
    for chunk in record_ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        tx.execute(
            &format!("DELETE FROM record_deletions WHERE record_id IN ({placeholders})"),
            rusqlite::params_from_iter(chunk),
        )?;
        tx.execute(
            &format!("DELETE FROM records WHERE record_id IN ({placeholders})"),
            rusqlite::params_from_iter(chunk),
        )?;
    }
    let mut insert = tx
        .prepare_cached("INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) VALUES (?1, ?2, ?3)")?;
    for record_id in record_ids {
        insert.execute(params![record_id, erased_at, reason_code.as_str()])?;
    }
    Ok(())
}
