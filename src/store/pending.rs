//! Read pending attachment file work recorded by migration 005.
//!
//! `internal_pending_file_operations` holds recovery state for file changes
//! that a SQLite transaction cannot include. This module is the one reader of
//! that table. It only reads, so a read-only command can report unfinished work
//! without changing anything. `package::resume_pending_file_operations`
//! finishes the work; every command that opens a database for writing calls it.

use rusqlite::Connection;
use serde::Serialize;

use super::Store;
use crate::error::Result;

pub const PENDING_FILE_OPERATIONS_TABLE: &str = "internal_pending_file_operations";

/// One unfinished file change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingFileOperation {
    pub operation_id: String,
    /// `attachment_write` or `attachment_removal`.
    pub operation_kind: String,
    /// Relative to the folder that holds the database.
    pub relative_path: String,
    pub created_at: String,
}

/// Every pending file operation in a stable order. A database before SQL
/// schema step 5 has no such table and therefore no pending work.
pub(crate) fn pending_file_operations(connection: &Connection) -> Result<Vec<PendingFileOperation>> {
    let present: bool = connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [PENDING_FILE_OPERATIONS_TABLE],
        |row| row.get(0),
    )?;
    if !present {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT operation_id, operation_kind, relative_path, created_at
         FROM internal_pending_file_operations
         ORDER BY created_at, operation_id, relative_path",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PendingFileOperation {
            operation_id: row.get(0)?,
            operation_kind: row.get(1)?,
            relative_path: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

impl Store {
    /// Every pending file operation in a stable order.
    pub fn pending_file_operations(&self) -> Result<Vec<PendingFileOperation>> {
        pending_file_operations(self.connection())
    }
}

impl PendingFileOperation {
    /// A sentence for `ath check`. It says what the cleanup will do: an
    /// approved removal is finished, and a file an interrupted write left is
    /// removed once its writer has stopped. Either way a file that an
    /// attachment record names is kept.
    pub fn describe(&self) -> String {
        match self.operation_kind.as_str() {
            "attachment_removal" => format!(
                "an erasure has not finished removing attachment file {}. The next ath command that writes to this database removes it.",
                self.relative_path
            ),
            "attachment_write" => format!(
                "an interrupted attachment write left {}, which no record names. The next ath command that writes to this database removes it once the process that wrote it has stopped.",
                self.relative_path
            ),
            other => format!("file operation {other} is not finished: {}", self.relative_path),
        }
    }
}
