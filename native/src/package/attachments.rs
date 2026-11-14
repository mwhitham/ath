//! Attachment files: media rules, stored names, and recoverable file changes.
//!
//! An attachment record names a file in the `attachments` directory next to
//! the database. The file is stored as `<record ID><extension>`. Its size,
//! SHA-256, and media type must match the record.
//!
//! A SQLite transaction cannot include file changes, so file changes carry
//! durable recovery rows in `internal_pending_file_operations` (migration 005):
//!
//! - A write first commits one row per file it will create. It then writes and
//!   synchronizes the files, and commits the attachment records in the same
//!   transaction that deletes its rows. An interrupted write leaves rows that
//!   name only files nothing references. A later writable operation removes
//!   those files once their owner has stopped.
//! - An erasure commits one removal row per attachment file in the same
//!   transaction that erases the records. It then removes the files and
//!   deletes the rows. A removal that fails stays listed until it succeeds.
//!
//! Rows hold an operation ID, a kind, a path relative to the database folder,
//! and a time. They hold no health values and are never exported.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use super::failpoint;
use crate::error::{Error, Result};
use crate::files::{self, durable, process};
use crate::input_policy::MAX_ATTACHMENT_BYTES;
use crate::model::*;
use crate::store::{self, Applied, Changes, NewRecord, Store};

/// The folder next to the database that holds attachment files.
pub const ATTACHMENT_DIRECTORY: &str = "attachments";

/// The attachment folder for a database file.
pub fn attachment_directory(database_path: &Path) -> PathBuf {
    durable::parent_of(database_path).join(ATTACHMENT_DIRECTORY)
}

/// The media type that a file's leading bytes show: PNG, JPEG, WebP, or PDF.
pub fn sniff_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if is_webp(bytes) {
        return Some("image/webp");
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("application/pdf");
    }
    None
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.get(0..4) == Some(b"RIFF".as_slice()) && bytes.get(8..12) == Some(b"WEBP".as_slice())
}

/// Refuse bytes that do not match the record's media type. PNG, JPEG, WebP,
/// and PDF must start with their signature. Any other image type is refused.
/// A non-image type other than PDF is accepted as stated.
pub fn check_attachment_bytes(record_id: &str, data: &AttachmentData, bytes: &[u8]) -> Result<()> {
    let media_type = data.media_type.to_lowercase();
    let matches = match media_type.as_str() {
        "image/png" => bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => is_webp(bytes),
        "application/pdf" => bytes.starts_with(b"%PDF-"),
        other => !other.starts_with("image/"),
    };
    if !matches {
        return Err(Error::refused(format!(
            "attachment bytes do not match supported media type {}: {record_id}",
            data.media_type
        )));
    }
    Ok(())
}

/// The stored file name: the record ID plus an extension. A known media type
/// sets the extension. Otherwise a short alphanumeric extension of the
/// original file name is kept in lowercase, and any other extension is dropped.
pub fn stored_attachment_name(record_id: &str, data: &AttachmentData) -> String {
    let known = match data.media_type.as_str() {
        "image/jpeg" => Some(".jpg"),
        "image/png" => Some(".png"),
        "image/webp" => Some(".webp"),
        "application/pdf" => Some(".pdf"),
        _ => None,
    };
    let extension = match known {
        Some(extension) => extension.to_string(),
        None => {
            let input = posix_extname(&data.file_name);
            let tail = input.strip_prefix('.').unwrap_or_default();
            if input.starts_with('.')
                && (1..=10).contains(&tail.len())
                && tail.bytes().all(|b| b.is_ascii_alphanumeric())
            {
                input.to_ascii_lowercase()
            } else {
                String::new()
            }
        }
    };
    format!("{record_id}{extension}")
}

/// `path.posix.extname`: the last `.` and what follows it in the last path
/// segment, except for a segment that starts with its only dot. Slashes and
/// dots are ASCII, so working on UTF-8 bytes gives the same split as UTF-16.
fn posix_extname(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut start_dot: Option<usize> = None;
    let mut start_part = 0usize;
    let mut end: Option<usize> = None;
    let mut matched_slash = true;
    // 0: dot is the first character of the segment; 1: another dot follows; -1: other text.
    let mut pre_dot_state = 0i8;
    for index in (0..bytes.len()).rev() {
        let byte = bytes[index];
        if byte == b'/' {
            if !matched_slash {
                start_part = index + 1;
                break;
            }
            continue;
        }
        if end.is_none() {
            matched_slash = false;
            end = Some(index + 1);
        }
        if byte == b'.' {
            if start_dot.is_none() {
                start_dot = Some(index);
            } else if pre_dot_state != 1 {
                pre_dot_state = 1;
            }
        } else if start_dot.is_some() {
            pre_dot_state = -1;
        }
    }
    let (Some(start_dot), Some(end)) = (start_dot, end) else {
        return "";
    };
    if pre_dot_state == 0 || (pre_dot_state == 1 && start_dot == end - 1 && start_dot == start_part + 1) {
        return "";
    }
    &path[start_dot..end]
}

/// `attachments/<stored name>`.
pub fn relative_attachment_path(stored_name: &str) -> String {
    format!("{ATTACHMENT_DIRECTORY}/{stored_name}")
}

// ---- Ingestion for ath log and ath correct ------------------------------

/// An attachment read and checked before anything is written.
#[derive(Debug, Clone)]
pub struct PreparedAttachment {
    pub record_id: String,
    pub created_at: String,
    pub changed_by: RecordActor,
    pub data: AttachmentData,
    /// Where the file will be stored.
    pub final_path: PathBuf,
    bytes: Vec<u8>,
}

impl PreparedAttachment {
    /// The attachment record to include in the same change.
    pub fn new_record(&self) -> NewRecord {
        NewRecord {
            data: RecordData::Attachment(self.data.clone()),
            extensions: None,
            record_id: Some(self.record_id.clone()),
            created_at: Some(self.created_at.clone()),
            became_known_at: None,
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            changed_by: self.changed_by.clone(),
        }
    }

    pub fn stored_name(&self) -> String {
        stored_attachment_name(&self.record_id, &self.data)
    }
}

/// Read and check attachment files for a later write. Each file must be a
/// regular file reached without a symbolic link, at most 25 MiB, and a PNG,
/// JPEG, WebP, or PDF by its leading bytes. Nothing is written.
pub fn prepare_attachments(
    store: &Store,
    paths: &[PathBuf],
    changed_by: &RecordActor,
    created_at: &str,
) -> Result<Vec<PreparedAttachment>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let directory = attachment_directory(store.path());
    files::refuse_symbolic_link_components(&directory, true)?;
    let mut prepared = Vec::new();
    for supplied in paths {
        let path = files::absolute(supplied)?;
        files::refuse_symbolic_link_components(&path, false)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            return Err(Error::refused(format!("attachment is not a regular file: {}", path.display())));
        }
        if metadata.len() > MAX_ATTACHMENT_BYTES {
            return Err(Error::refused(format!("attachment exceeds {MAX_ATTACHMENT_BYTES} bytes: {}", path.display())));
        }
        let bytes = fs::read(&path)?;
        let Some(media_type) = sniff_media_type(&bytes) else {
            return Err(Error::refused(format!(
                "unsupported attachment type: {}; use PNG, JPEG, WebP, or PDF",
                path.display()
            )));
        };
        let record_id = crate::ids::new_record_id();
        let data = AttachmentData {
            file_name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            media_type: media_type.to_string(),
            byte_count: bytes.len() as u64,
            sha256: durable::sha256_hex(&bytes),
            description: None,
        };
        let final_path = directory.join(stored_attachment_name(&record_id, &data));
        prepared.push(PreparedAttachment {
            record_id,
            created_at: created_at.to_string(),
            changed_by: changed_by.clone(),
            data,
            final_path,
            bytes,
        });
    }
    Ok(prepared)
}

/// Write attachment files and commit a change that records them, in that
/// order. `changes` must create each prepared attachment's record (see
/// `PreparedAttachment::new_record`). If the change fails, the new files are
/// removed. If this process stops first, a later writable operation removes
/// them (see `resume_pending_file_operations`).
pub fn apply_with_attachments(
    store: &mut Store,
    attachments: &[PreparedAttachment],
    changes: Changes,
) -> Result<Applied> {
    if attachments.is_empty() {
        return store.apply(changes);
    }
    for attachment in attachments {
        let matching = changes
            .creates
            .iter()
            .filter(|create| {
                create.record_id.as_deref() == Some(attachment.record_id.as_str())
                    && create.data == RecordData::Attachment(attachment.data.clone())
            })
            .count();
        if matching != 1 {
            return Err(Error::other(format!(
                "the change must create attachment record {} exactly once",
                attachment.record_id
            )));
        }
    }
    let files: Vec<NewAttachmentFile> = attachments
        .iter()
        .map(|attachment| NewAttachmentFile {
            stored_name: attachment.stored_name(),
            source: AttachmentSource::Bytes(attachment.bytes.clone()),
            byte_count: attachment.data.byte_count,
            sha256: attachment.data.sha256.clone(),
        })
        .collect();
    let mut batch = AttachmentFileBatch::begin(store, files)?;
    let layout = store.layout();
    let result = store.transaction(|tx| {
        let applied = store::write::apply_in(tx, layout, changes)?;
        batch.finish_in(tx)?;
        failpoint!("attachments.before_commit");
        Ok(applied)
    });
    match result {
        Ok(applied) => {
            batch.committed();
            Ok(applied)
        }
        Err(error) => {
            batch.abort(store);
            Err(error)
        }
    }
}

// ---- Owned file writes ---------------------------------------------------

/// Where the bytes of a new attachment file come from.
#[allow(dead_code)] // Package restore uses the file source.
pub(crate) enum AttachmentSource {
    Bytes(Vec<u8>),
    /// A package file. It is read and checked again when copied.
    File(PathBuf),
}

pub(crate) struct NewAttachmentFile {
    pub stored_name: String,
    pub source: AttachmentSource,
    pub byte_count: u64,
    pub sha256: String,
}

/// Operations of this process that are still running. Another process's
/// operations are running while its process is.
static IN_FLIGHT: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn new_operation_id() -> String {
    format!("{}-{}", process::current_pid(), uuid::Uuid::now_v7().simple())
}

fn operation_owner(operation_id: &str) -> Option<u32> {
    operation_id.split_once('-').and_then(|(pid, _)| pid.parse().ok())
}

/// True when the operation's owner may still be working on it.
fn operation_is_active(operation_id: &str) -> bool {
    match operation_owner(operation_id) {
        Some(pid) if pid == process::current_pid() => {
            IN_FLIGHT.lock().map(|set| set.contains(operation_id)).unwrap_or(true)
        }
        Some(pid) => process::is_process_alive(pid),
        // An ID this library did not write: keep the files and report them.
        None => true,
    }
}

struct InFlight(String);

impl InFlight {
    fn register(operation_id: &str) -> Self {
        if let Ok(mut set) = IN_FLIGHT.lock() {
            set.insert(operation_id.to_string());
        }
        InFlight(operation_id.to_string())
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if let Ok(mut set) = IN_FLIGHT.lock() {
            set.remove(&self.0);
        }
    }
}

/// New attachment files owned by one operation until its records commit.
pub(crate) struct AttachmentFileBatch {
    operation_id: String,
    directory: PathBuf,
    /// Relative paths recorded as pending, final names and temporary names.
    recorded: Vec<String>,
    /// Final files this operation created, with their expected sizes.
    created: Vec<(PathBuf, u64)>,
    _in_flight: InFlight,
}

impl AttachmentFileBatch {
    /// Record the pending files durably, then write and synchronize them.
    pub(crate) fn begin(store: &mut Store, files: Vec<NewAttachmentFile>) -> Result<Self> {
        let operation_id = new_operation_id();
        let in_flight = InFlight::register(&operation_id);
        let directory = attachment_directory(store.path());
        files::refuse_symbolic_link_components(&directory, true)?;
        files::create_private_dir_all(&directory)?;
        files::refuse_symbolic_link_components(&directory, false)?;
        let temporary_suffix = format!(".tmp-{operation_id}");
        let mut recorded = Vec::new();
        for file in &files {
            recorded.push(relative_attachment_path(&file.stored_name));
            recorded.push(relative_attachment_path(&format!("{}{temporary_suffix}", file.stored_name)));
        }
        let mut batch =
            AttachmentFileBatch { operation_id, directory, recorded, created: Vec::new(), _in_flight: in_flight };
        let now = time::now_utc();
        store.transaction(|tx| {
            let mut insert = tx.prepare_cached(
                "INSERT INTO internal_pending_file_operations (operation_id, operation_kind, relative_path, created_at)
                 VALUES (?1, 'attachment_write', ?2, ?3)",
            )?;
            for path in &batch.recorded {
                insert.execute(params![batch.operation_id, path, now])?;
            }
            Ok(())
        })?;
        if let Err(error) = batch.write_files(files, &temporary_suffix) {
            batch.abort(store);
            return Err(error);
        }
        Ok(batch)
    }

    fn write_files(&mut self, files: Vec<NewAttachmentFile>, temporary_suffix: &str) -> Result<()> {
        failpoint!("attachments.after_pending_rows");
        for file in files {
            let final_path = self.directory.join(&file.stored_name);
            if fs::symlink_metadata(&final_path).is_ok() {
                return Err(Error::refused(format!("attachment file already exists: {}", final_path.display())));
            }
            let bytes = match file.source {
                AttachmentSource::Bytes(bytes) => bytes,
                AttachmentSource::File(path) => {
                    files::refuse_symbolic_link_components(&path, false)?;
                    fs::read(&path)?
                }
            };
            if bytes.len() as u64 != file.byte_count || durable::sha256_hex(&bytes) != file.sha256 {
                return Err(Error::refused(format!(
                    "attachment bytes changed before they were copied: {}",
                    file.stored_name
                )));
            }
            let temporary = self.directory.join(format!("{}{temporary_suffix}", file.stored_name));
            {
                let mut handle = files::create_new_private(&temporary)?;
                handle.write_all(&bytes)?;
                handle.sync_all()?;
            }
            failpoint!("attachments.after_temporary_write");
            if fs::symlink_metadata(&final_path).is_ok() {
                let _ = fs::remove_file(&temporary);
                return Err(Error::refused(format!("attachment file already exists: {}", final_path.display())));
            }
            fs::rename(&temporary, &final_path)?;
            self.created.push((final_path, file.byte_count));
            failpoint!("attachments.after_rename");
        }
        files::sync_directory(&self.directory)?;
        failpoint!("attachments.after_directory_sync");
        Ok(())
    }

    /// Inside the transaction that records the attachments: check that every
    /// file is still present and still owned, then delete the pending rows.
    pub(crate) fn finish_in(&self, tx: &Transaction<'_>) -> Result<()> {
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM internal_pending_file_operations WHERE operation_id = ?1",
            [&self.operation_id],
            |row| row.get(0),
        )?;
        if remaining as usize != self.recorded.len() {
            return Err(Error::stale(
                "attachment files were cleaned up before their records were written; run the command again",
            ));
        }
        for (path, byte_count) in &self.created {
            let metadata = fs::symlink_metadata(path).map_err(|_| {
                Error::stale(format!("attachment file disappeared before its record was written: {}", path.display()))
            })?;
            if !metadata.is_file() || metadata.len() != *byte_count {
                return Err(Error::stale(format!(
                    "attachment file changed before its record was written: {}",
                    path.display()
                )));
            }
        }
        tx.execute("DELETE FROM internal_pending_file_operations WHERE operation_id = ?1", [&self.operation_id])?;
        Ok(())
    }

    /// The records committed; the files now belong to them.
    pub(crate) fn committed(self) {}

    /// Remove the files this operation created and its pending rows. A step
    /// that fails leaves the rows, so a later writable operation retries.
    pub(crate) fn abort(&mut self, store: &mut Store) {
        let mut removed_all = true;
        for relative in &self.recorded {
            let Some(name) = relative.strip_prefix(&format!("{ATTACHMENT_DIRECTORY}/")) else { continue };
            if durable::remove_file_if_present(&self.directory.join(name)).is_err() {
                removed_all = false;
            }
        }
        let _ = files::sync_directory(&self.directory);
        if removed_all {
            let operation_id = self.operation_id.clone();
            let _ = store.transaction(|tx| {
                tx.execute("DELETE FROM internal_pending_file_operations WHERE operation_id = ?1", [&operation_id])?;
                Ok(())
            });
        }
        self.created.clear();
    }
}

// ---- Pending operations: report and resume --------------------------------

/// One pending file change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingFileOperation {
    pub operation_id: String,
    pub operation_kind: String,
    pub relative_path: String,
    pub created_at: String,
}

/// Pending file changes in a database, oldest first. A database before SQL
/// schema 5 has none. This read changes nothing.
pub fn pending_file_operations(connection: &Connection) -> Result<Vec<PendingFileOperation>> {
    if !has_pending_table(connection)? {
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

fn has_pending_table(connection: &Connection) -> Result<bool> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'internal_pending_file_operations'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// What a cleanup pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PendingCleanupReport {
    /// Files removed, relative to the database folder.
    pub removed_files: Vec<String>,
    /// Pending entries closed without removing their file because a record
    /// references it.
    pub retained_files: Vec<String>,
    /// Operations left alone because their owner is still running.
    pub active_operations: Vec<String>,
    /// Files that could not be removed. Their entries stay pending.
    pub failed_files: Vec<PendingFileFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingFileFailure {
    pub relative_path: String,
    pub message: String,
}

impl PendingCleanupReport {
    pub fn is_complete(&self) -> bool {
        self.failed_files.is_empty() && self.active_operations.is_empty()
    }
}

/// Finish file changes that earlier operations left pending: removals an
/// erasure approved, and files an interrupted write created but never
/// recorded. Files a record references are kept. Operations whose owner is
/// still running are left alone. Call this at the start of a writable command.
pub fn resume_pending_file_operations(store: &mut Store) -> Result<PendingCleanupReport> {
    if store.is_read_only() || !has_pending_table(store.connection())? {
        return Ok(PendingCleanupReport::default());
    }
    if pending_file_operations(store.connection())?.is_empty() {
        return Ok(PendingCleanupReport::default());
    }
    let directory = attachment_directory(store.path());
    store.transaction(|tx| {
        let pending = pending_file_operations(tx)?;
        let mut by_operation: BTreeMap<String, Vec<PendingFileOperation>> = BTreeMap::new();
        for entry in pending {
            by_operation.entry(entry.operation_id.clone()).or_default().push(entry);
        }
        let mut report = PendingCleanupReport::default();
        for (operation_id, entries) in by_operation {
            let writes = entries.iter().any(|entry| entry.operation_kind == "attachment_write");
            if writes && operation_is_active(&operation_id) {
                report.active_operations.push(operation_id);
                continue;
            }
            for entry in entries {
                match settle_entry(tx, &directory, &entry) {
                    Ok(Settled::Removed) => report.removed_files.push(entry.relative_path.clone()),
                    Ok(Settled::Absent) => {}
                    Ok(Settled::Referenced) => report.retained_files.push(entry.relative_path.clone()),
                    Err(error) => {
                        report.failed_files.push(PendingFileFailure {
                            relative_path: entry.relative_path,
                            message: error.to_string(),
                        });
                        continue;
                    }
                }
                tx.execute(
                    "DELETE FROM internal_pending_file_operations WHERE operation_id = ?1 AND relative_path = ?2",
                    params![entry.operation_id, entry.relative_path],
                )?;
            }
        }
        if !report.removed_files.is_empty() && directory.is_dir() {
            files::sync_directory(&directory)?;
        }
        Ok(report)
    })
}

enum Settled {
    Removed,
    Absent,
    Referenced,
}

/// The single file name in an `attachments/<name>` path. Anything else is refused.
pub(crate) fn pending_file_name(relative_path: &str) -> Result<&str> {
    let refused = || Error::refused(format!("pending file path is outside the attachment folder: {relative_path}"));
    let name = relative_path.strip_prefix(&format!("{ATTACHMENT_DIRECTORY}/")).ok_or_else(refused)?;
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(part)), None) if part.to_str() == Some(name) && !name.contains('\\') => Ok(name),
        _ => Err(refused()),
    }
}

/// True when an attachment record stores its file under this name.
pub(crate) fn is_referenced(connection: &Connection, name: &str) -> Result<bool> {
    let Some(record_id) = name.get(..36) else { return Ok(false) };
    if !is_record_id(record_id) {
        return Ok(false);
    }
    let row: Option<(String, String)> = connection
        .query_row("SELECT file_name, media_type FROM attachments WHERE record_id = ?1", [record_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let Some((file_name, media_type)) = row else { return Ok(false) };
    let data = AttachmentData { file_name, media_type, byte_count: 0, sha256: String::new(), description: None };
    Ok(stored_attachment_name(record_id, &data) == name)
}

fn settle_entry(tx: &Transaction<'_>, directory: &Path, entry: &PendingFileOperation) -> Result<Settled> {
    let name = pending_file_name(&entry.relative_path)?;
    if is_referenced(tx, name)? {
        return Ok(Settled::Referenced);
    }
    let path = directory.join(name);
    files::refuse_symbolic_link_components(&path, true)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() => {
            Err(Error::refused(format!("pending attachment path is not a regular file: {}", path.display())))
        }
        Ok(_) => {
            fs::remove_file(&path)?;
            Ok(Settled::Removed)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settled::Absent),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(file_name: &str, media_type: &str) -> AttachmentData {
        AttachmentData {
            file_name: file_name.into(),
            media_type: media_type.into(),
            byte_count: 0,
            sha256: "0".repeat(64),
            description: None,
        }
    }

    #[test]
    fn extname_follows_node_posix_rules() {
        for (input, expected) in [
            ("index.html", ".html"),
            ("index.coffee.md", ".md"),
            ("index.", "."),
            ("index", ""),
            (".index", ""),
            (".index.md", ".md"),
            ("..", ""),
            ("a/b.c/d", ""),
            ("a/.b", ""),
            ("a.png/", ".png"),
            ("..png", ".png"),
            ("ä.Tiff", ".Tiff"),
        ] {
            assert_eq!(posix_extname(input), expected, "{input}");
        }
    }

    #[test]
    fn stored_names_use_known_or_short_extensions() {
        let id = "01890f3e-e7d3-7cc0-88c8-00000000000e";
        assert_eq!(stored_attachment_name(id, &data("x.PNG", "image/png")), format!("{id}.png"));
        assert_eq!(stored_attachment_name(id, &data("x.HEIC", "image/heic")), format!("{id}.heic"));
        assert_eq!(stored_attachment_name(id, &data("x.12345678901", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("x.t-t", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("notes", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("x.jpeg", "IMAGE/JPEG")), format!("{id}.jpeg"));
    }

    #[test]
    fn media_types_are_sniffed_and_checked() {
        let png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0];
        assert_eq!(sniff_media_type(&png), Some("image/png"));
        assert_eq!(sniff_media_type(&[0xff, 0xd8, 0xff, 0]), Some("image/jpeg"));
        assert_eq!(sniff_media_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_media_type(b"%PDF-1.7"), Some("application/pdf"));
        assert_eq!(sniff_media_type(b"GIF89a"), None);
        assert_eq!(sniff_media_type(b"RIFF"), None);
        let id = "01890f3e-e7d3-7cc0-88c8-00000000000e";
        assert!(check_attachment_bytes(id, &data("a.png", "image/png"), &png).is_ok());
        assert!(check_attachment_bytes(id, &data("a.png", "Image/PNG"), &png).is_ok());
        assert!(check_attachment_bytes(id, &data("a.jpg", "image/jpeg"), &png).is_err());
        assert!(check_attachment_bytes(id, &data("a.gif", "image/gif"), b"GIF89a").is_err());
        assert!(check_attachment_bytes(id, &data("a.txt", "text/plain"), b"hello").is_ok());
        let error = check_attachment_bytes(id, &data("a.pdf", "application/pdf"), b"nope").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("attachment bytes do not match supported media type application/pdf: {id}")
        );
    }

    #[test]
    fn pending_paths_must_name_one_attachment_file() {
        assert_eq!(pending_file_name("attachments/a.png").unwrap(), "a.png");
        for bad in [
            "attachments/../x",
            "attachments/a/b",
            "x/a.png",
            "attachments/",
            "attachments/..",
            "/etc/passwd",
            "attachments/a\\b",
        ] {
            assert!(pending_file_name(bad).is_err(), "{bad}");
        }
    }
}
