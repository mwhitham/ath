//! `ath repair`: rebuild typed rows from the retained current revision.
//!
//! Two kinds of damage are repaired:
//!
//! - Encoding: a typed row's `data_json` differs from the current revision
//!   only in key order, or its typed columns and child rows disagree while the
//!   JSON text matches. npm 1.0.0 provider imports left the first kind.
//! - Revision alignment (`--from-revision`): the typed row disagrees with the
//!   current revision in meaning. The current revision is the record; the
//!   typed row is rebuilt from it after the person confirms.
//!
//! Every repair makes a verified backup first and runs in one transaction.
//! It uses the record encoder through `encode::rebuild_from_current_revision`.
//! A storage repair never creates a revision, so it never looks like an
//! athlete correction. It advances the database revision once when it
//! changes rows.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;

use super::check::{typed_denormalized_findings, TypedDenormalizedFinding};
use super::encode::{self, Layout};
use super::{backup, database_revision, read_metadata, Store};
use crate::error::{Error, Result};
use crate::files;
use crate::json;
use crate::model::{time, RecordType};

/// A typed row whose `data_json` differs from the current revision's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTableMismatch {
    pub record_id: String,
    pub record_type: String,
    /// True when the two texts hold the same JSON value in a different key order.
    pub encoding_only: bool,
}

/// What a repair would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairCandidates {
    /// Records an encoding repair rebuilds, in repair order.
    pub encoding_record_ids: Vec<String>,
    /// Records whose typed rows disagree with the current revision in meaning.
    pub revision_align_record_ids: Vec<String>,
    /// Every `data_json` difference, including encoding-only ones.
    pub typed_table_mismatches: usize,
}

impl RepairCandidates {
    pub fn repair_candidates(&self) -> usize {
        self.encoding_record_ids.len()
    }

    pub fn revision_align_candidates(&self) -> usize {
        self.revision_align_record_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.typed_table_mismatches == 0 && self.encoding_record_ids.is_empty()
    }
}

/// The `ath repair` preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepairPreview {
    pub file: String,
    pub repair_candidates: usize,
    pub revision_align_candidates: usize,
    pub revision_align_record_ids: Vec<String>,
    pub write_confirmed: bool,
}

/// Options for a confirmed repair.
#[derive(Debug, Clone, Default)]
pub struct RepairOptions {
    /// Also rebuild typed rows that disagree with the current revision in meaning.
    pub from_revision: bool,
    pub backup_path: Option<PathBuf>,
    /// The time used to name a default backup.
    pub now_millis: Option<i64>,
}

/// The result of a confirmed repair. `backup_path` is empty when nothing
/// needed repair and no backup was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfirmedRepairResult {
    pub backup_path: String,
    pub repaired_record_count: usize,
    pub revision_align_record_count: usize,
    pub database_revision: u64,
}

/// The result of `repair_provider_encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderEncodingRepairResult {
    pub repaired_record_count: usize,
    pub database_revision: u64,
}

fn semantic_json_equal(left: &str, right: &str) -> bool {
    match (json::parse(left), json::parse(right)) {
        (Ok(left), Ok(right)) => json::semantic_eq(&left, &right),
        _ => false,
    }
}

/// Every typed row whose `data_json` differs from its current revision, by
/// record type and then record ID. Deleted records are included.
pub fn typed_table_mismatches(connection: &Connection) -> Result<Vec<TypedTableMismatch>> {
    let mut mismatches = Vec::new();
    for record_type in RecordType::ALL {
        let table = encode::table_for(*record_type);
        let mut statement = connection.prepare(&format!(
            "SELECT records.record_id, record_revisions.record_json, {table}.data_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             JOIN {table} ON {table}.record_id = records.record_id
             WHERE records.record_type = ?1
               AND {table}.data_json <> record_revisions.record_json
             ORDER BY records.record_id"
        ))?;
        let mut rows = statement.query([record_type.as_str()])?;
        while let Some(row) = rows.next()? {
            let record_id: String = row.get(0)?;
            let record_json: String = row.get(1)?;
            let typed_json: String = row.get(2)?;
            mismatches.push(TypedTableMismatch {
                record_id,
                record_type: record_type.as_str().to_string(),
                encoding_only: semantic_json_equal(&record_json, &typed_json),
            });
        }
    }
    Ok(mismatches)
}

/// Find what a repair would change.
pub fn repair_candidates(connection: &Connection, layout: Layout) -> Result<RepairCandidates> {
    let mismatches = typed_table_mismatches(connection)?;
    let revision_align_record_ids: Vec<String> =
        mismatches.iter().filter(|row| !row.encoding_only).map(|row| row.record_id.clone()).collect();
    let mut encoding_record_ids: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in mismatches.iter().filter(|row| row.encoding_only) {
        if seen.insert(row.record_id.clone()) {
            encoding_record_ids.push(row.record_id.clone());
        }
    }
    let aligned: std::collections::HashSet<&String> = revision_align_record_ids.iter().collect();
    for finding in typed_denormalized_findings(connection, layout)? {
        if let TypedDenormalizedFinding::Mismatch { record_id } = finding {
            if !aligned.contains(&record_id) && seen.insert(record_id.clone()) {
                encoding_record_ids.push(record_id);
            }
        }
    }
    Ok(RepairCandidates { encoding_record_ids, revision_align_record_ids, typed_table_mismatches: mismatches.len() })
}

/// The `ath repair` preview for an open database. It changes nothing.
pub fn repair_preview(store: &Store) -> Result<RepairPreview> {
    let candidates = repair_candidates(store.connection(), store.layout())?;
    Ok(RepairPreview {
        file: store.path().display().to_string(),
        repair_candidates: candidates.repair_candidates(),
        revision_align_candidates: candidates.revision_align_candidates(),
        revision_align_record_ids: candidates.revision_align_record_ids,
        write_confirmed: false,
    })
}

/// Rebuild typed rows from current revisions. Without `allow_semantic`, a
/// row whose text differs in meaning stops the repair.
pub(crate) fn rebuild_typed_rows(
    tx: &Transaction<'_>,
    layout: Layout,
    record_ids: &[String],
    allow_semantic: bool,
) -> Result<usize> {
    let mut repaired = 0;
    for record_id in record_ids {
        let record_type: Option<String> = tx
            .query_row("SELECT record_type FROM records WHERE record_id = ?1", [record_id], |row| row.get(0))
            .optional()?;
        let record_type = record_type.unwrap_or_else(|| "undefined".to_string());
        let parsed = RecordType::parse(&record_type)
            .ok_or_else(|| Error::other(format!("record {record_id} has unsupported record type {record_type}")))?;
        let table = encode::table_for(parsed);
        let texts: Option<(String, String)> = tx
            .query_row(
                &format!(
                    "SELECT record_revisions.record_json, {table}.data_json
                     FROM records
                     JOIN record_revisions
                       ON record_revisions.record_id = records.record_id
                      AND record_revisions.revision = records.current_revision
                     JOIN {table} ON {table}.record_id = records.record_id
                     WHERE records.record_id = ?1"
                ),
                [record_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((record_json, typed_json)) = texts else {
            return Err(Error::other(format!("record {record_id} is missing typed-table data for repair")));
        };
        if record_json != typed_json && !allow_semantic && !semantic_json_equal(&record_json, &typed_json) {
            return Err(Error::other(format!(
                "record {record_id} has a typed-table mismatch that is not key-order only; run ath repair --from-revision after reviewing affected records, or restore from backup"
            )));
        }
        encode::rebuild_from_current_revision(tx, layout, record_id)?;
        repaired += 1;
    }
    Ok(repaired)
}

fn bump_revision(tx: &Transaction<'_>) -> Result<u64> {
    tx.execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])?;
    database_revision(tx)
}

/// Repair encoding-only differences without a backup, as 2.1.0's
/// `repairProviderEncoding` did. Prefer `execute_confirmed_repair`.
pub fn repair_provider_encoding(store: &mut Store) -> Result<ProviderEncodingRepairResult> {
    let layout = store.layout();
    store.transaction(|tx| {
        // Every data_json difference is rebuilt. One that differs in meaning
        // stops the repair.
        let ids: Vec<String> = typed_table_mismatches(tx)?.into_iter().map(|row| row.record_id).collect();
        let repaired = rebuild_typed_rows(tx, layout, &ids, false)?;
        let database_revision = if repaired == 0 { database_revision(tx)? } else { bump_revision(tx)? };
        Ok(ProviderEncodingRepairResult { repaired_record_count: repaired, database_revision })
    })
}

/// Make a verified backup, then rebuild typed rows in one transaction.
///
/// Meaning differences need `from_revision`. Under the write lock, the
/// database revision must still match the backup; otherwise nothing is
/// repaired and the caller must preview again.
pub fn execute_confirmed_repair(store: &mut Store, options: &RepairOptions) -> Result<ConfirmedRepairResult> {
    if store.is_read_only() {
        return Err(Error::refused("This database was opened read-only"));
    }
    let layout = store.layout();
    let path = store.path().to_path_buf();
    let metadata = read_metadata(store.connection())?;
    let candidates = repair_candidates(store.connection(), layout)?;
    if candidates.is_empty() {
        return Ok(ConfirmedRepairResult {
            backup_path: String::new(),
            repaired_record_count: 0,
            revision_align_record_count: 0,
            database_revision: database_revision(store.connection())?,
        });
    }
    if !candidates.revision_align_record_ids.is_empty() && !options.from_revision {
        return Err(Error::refused(format!(
            "{} record(s) have typed-table bytes that disagree with the current revision in meaning ({}). Pass --from-revision and --confirm-revision-align with the database ID after reviewing the preview.",
            candidates.revision_align_record_ids.len(),
            candidates.revision_align_record_ids.join(", ")
        )));
    }

    let revision_before = database_revision(store.connection())?;
    let backup_path = options.backup_path.clone().unwrap_or_else(|| {
        backup::default_repair_backup_path(&path, options.now_millis.unwrap_or_else(time::now_millis))
    });
    let identity = files::identity(&path)?;
    let verified = backup::create_verified_backup(store.connection(), &backup_path, &metadata)?;
    if verified.database_revision != revision_before {
        return Err(Error::stale("Database revision changed during repair backup; stop other writers and retry"));
    }
    if files::identity(&path)? != identity {
        return Err(Error::refused("Database file changed during repair backup"));
    }
    let backup_text = verified.path.display().to_string();
    let stale = format!(
        "Database revision changed before repair could start. Nothing was repaired. The backup at {backup_text} is older than the database, so preview the repair again."
    );

    let outcome = store.transaction(|tx| {
        files::assert_same_file(&path, identity).map_err(|_| Error::stale(stale.clone()))?;
        if database_revision(tx)? != verified.database_revision {
            return Err(Error::stale(stale.clone()));
        }
        let aligned = rebuild_typed_rows(tx, layout, &candidates.revision_align_record_ids, true)?;
        let repaired = rebuild_typed_rows(tx, layout, &candidates.encoding_record_ids, false)?;
        let database_revision = if aligned + repaired == 0 { database_revision(tx)? } else { bump_revision(tx)? };
        Ok(ConfirmedRepairResult {
            backup_path: backup_text.clone(),
            repaired_record_count: repaired,
            revision_align_record_count: aligned,
            database_revision,
        })
    });
    match outcome {
        Ok(result) => Ok(result),
        Err(Error::Stale(message)) if message == stale => Err(Error::Stale(message)),
        Err(error) => {
            let changed =
                database_revision(store.connection()).map(|revision| revision != revision_before).unwrap_or(false);
            let tail = if changed {
                " The database revision changed; restore from the backup.".to_string()
            } else {
                format!(" Retry with a new backup; previous attempt left {backup_text} on disk.")
            };
            Err(Error::other(format!(
                "Repair failed after creating verified backup at {backup_text}. Cause: {error}.{tail}"
            )))
        }
    }
}
