//! What a host keeps inside an athlete's database: write receipts, the access epoch,
//! and the change log (D261–D263). The tables come from migration step 7. A database
//! that never connects to a host leaves the receipts empty and the epoch at 0.

use rusqlite::{OptionalExtension, Transaction};

use super::{Applied, Store};
use crate::error::{Error, Result};
use crate::model::time;

/// The host request that made a change. The digest is a keyed hash of the request
/// body; the body itself is never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReceipt {
    pub principal_id: String,
    pub client_id: String,
    pub request_id: String,
    pub operation: String,
    pub request_digest: String,
}

/// A receipt as stored, with the result the request had.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredReceipt {
    pub operation: String,
    pub request_digest: String,
    /// `{"database_revision": N, "records": [{"record_id": …, "revision": …}]}`
    pub result: serde_json::Value,
    pub database_revision: u64,
}

/// One change-log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedChange {
    pub database_revision: u64,
    pub record_id: String,
    /// 0 for an erasure.
    pub record_revision: u64,
    /// `revision`, `deletion`, or `erasure`.
    pub kind: String,
}

/// The changes after a database revision. When `complete` is false the log does not
/// reach back that far, so a copy at `from` needs a full package instead (D263).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesSince {
    pub from: u64,
    pub to: u64,
    pub complete: bool,
    pub changes: Vec<LoggedChange>,
}

pub(crate) fn check_access_epoch(tx: &Transaction<'_>, expected: u64) -> Result<()> {
    let epoch: i64 =
        tx.query_row("SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1", [], |row| row.get(0))?;
    if epoch as u64 != expected {
        return Err(Error::stale(
            "Access to this record changed while the request was waiting; check the grant and try again",
        ));
    }
    Ok(())
}

pub(crate) fn insert_receipt(tx: &Transaction<'_>, receipt: &WriteReceipt, applied: &Applied) -> Result<()> {
    let result = serde_json::json!({
        "database_revision": applied.database_revision,
        "records": applied
            .records
            .iter()
            .map(|record| serde_json::json!({ "record_id": record.record_id, "revision": record.current_revision }))
            .collect::<Vec<_>>(),
    });
    tx.execute(
        "INSERT INTO internal_write_receipts
           (principal_id, client_id, request_id, operation, request_digest, result_json, database_revision, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            receipt.principal_id,
            receipt.client_id,
            receipt.request_id,
            receipt.operation,
            receipt.request_digest,
            result.to_string(),
            applied.database_revision as i64,
            time::now_utc(),
        ],
    )
    .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(failure, _) if failure.code == rusqlite::ErrorCode::ConstraintViolation => {
            Error::stale("This request ID was already used")
        }
        other => other.into(),
    })?;
    Ok(())
}

impl Store {
    /// The receipt for a request, if the request was accepted before.
    pub fn find_receipt(&self, principal_id: &str, client_id: &str, request_id: &str) -> Result<Option<StoredReceipt>> {
        let row: Option<(String, String, String, i64)> = self
            .connection()
            .query_row(
                "SELECT operation, request_digest, result_json, database_revision FROM internal_write_receipts
                 WHERE principal_id = ?1 AND client_id = ?2 AND request_id = ?3",
                [principal_id, client_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        row.map(|(operation, request_digest, result, database_revision)| {
            Ok(StoredReceipt {
                operation,
                request_digest,
                result: serde_json::from_str(&result)
                    .map_err(|error| Error::other(format!("A stored receipt is not JSON: {error}")))?,
                database_revision: database_revision as u64,
            })
        })
        .transpose()
    }

    /// The access epoch. Revoking a grant raises it (D262).
    pub fn current_access_epoch(&self) -> Result<u64> {
        let epoch: i64 = self.connection().query_row(
            "SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(epoch as u64)
    }

    /// Raise the access epoch in its own write transaction and return the new value.
    /// The database revision does not change: no record changed.
    pub fn raise_access_epoch(&mut self) -> Result<u64> {
        self.transaction(|tx| {
            let changed = tx.execute(
                "UPDATE internal_access_state SET access_epoch = access_epoch + 1 WHERE singleton_id = 1",
                [],
            )?;
            if changed != 1 {
                return Err(Error::other("The access state row is missing"));
            }
            let epoch: i64 =
                tx.query_row("SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1", [], |row| {
                    row.get(0)
                })?;
            Ok(epoch as u64)
        })
    }

    /// The logged changes after `revision`, in order (D263).
    pub fn changes_since(&self, revision: u64) -> Result<ChangesSince> {
        let to = self.database_revision()?;
        if revision > to {
            return Err(Error::stale(format!("Revision {revision} is newer than this database's revision {to}")));
        }
        let mut statement = self.connection().prepare(
            "SELECT database_revision, record_id, record_revision, change_kind FROM internal_change_log
             WHERE database_revision > ?1 ORDER BY database_revision, record_id, change_kind, record_revision",
        )?;
        let changes: Vec<LoggedChange> = statement
            .query_map([revision as i64], |row| {
                Ok(LoggedChange {
                    database_revision: row.get::<_, i64>(0)? as u64,
                    record_id: row.get(1)?,
                    record_revision: row.get::<_, i64>(2)? as u64,
                    kind: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let oldest: Option<i64> =
            self.connection()
                .query_row("SELECT min(database_revision) FROM internal_change_log", [], |row| row.get(0))?;
        let complete = revision == to || oldest.is_some_and(|oldest| revision + 1 >= oldest as u64);
        Ok(ChangesSince { from: revision, to, complete, changes })
    }
}
