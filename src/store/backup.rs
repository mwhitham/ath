//! Verified SQLite backups for migration and repair.
//!
//! A backup is made with SQLite's in-process backup API. It needs no `sqlite3`
//! program. The copy is read from one snapshot of the source, so the database
//! revision and record count read in that snapshot describe the copy exactly.
//! The backup is then checked on its own: SQLite integrity, metadata, database
//! ID, schema step, database revision, and record count must all match. A
//! backup that fails a check is removed. An existing file is never replaced.
//!
//! The copy is switched to rollback-journal mode so that it is one
//! self-contained file. Restoring it means copying that one file back.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use super::{read_metadata, DatabaseMetadata, Store, SQLITE_APPLICATION_MARKER};
use crate::error::{Error, Result};
use crate::files;
use crate::model::time;

static BACKUP_NAME_SEQUENCE: AtomicU32 = AtomicU32::new(0);

/// 1, 2, … 9999, 0, 1, … across the process, as in 2.1.0.
fn next_sequence() -> u32 {
    (BACKUP_NAME_SEQUENCE.fetch_add(1, Ordering::SeqCst).wrapping_add(1)) % 10_000
}

/// `20260115T120000.123Z`: the UTC time with separators removed.
pub fn format_migration_backup_timestamp(now_millis: i64) -> String {
    let iso: String = time::format_utc_millis(now_millis).chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("{}Z", iso.replacen('Z', "", 1))
}

/// The same stamp from a JavaScript `Date.toISOString()` text, which can name
/// years outside 0–9999.
pub fn format_iso_backup_timestamp(iso: &str) -> String {
    let compact: String = iso.chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("{}Z", compact.replacen('Z', "", 1))
}

/// `<database>.migration-backup.<time>-<n>.sqlite`. The counter makes two
/// names planned in the same millisecond differ.
pub fn default_migration_backup_path(database_path: &Path, now_millis: i64) -> PathBuf {
    let stamp = format_migration_backup_timestamp(now_millis);
    files::sidecar(database_path, &format!(".migration-backup.{stamp}-{}.sqlite", next_sequence()))
}

/// The migration backup name for an ISO time and a counter value the caller
/// keeps. The JavaScript client keeps the counter across worker processes.
pub fn migration_backup_path(database_path: &Path, iso: &str, sequence: u32) -> PathBuf {
    let stamp = format_iso_backup_timestamp(iso);
    files::sidecar(database_path, &format!(".migration-backup.{stamp}-{sequence}.sqlite"))
}

/// `<database>.repair-backup.<time>-<n>.sqlite`.
pub fn default_repair_backup_path(database_path: &Path, now_millis: i64) -> PathBuf {
    let stamp = format_migration_backup_timestamp(now_millis);
    files::sidecar(database_path, &format!(".repair-backup.{stamp}-{}.sqlite", next_sequence()))
}

/// A backup that passed verification. It names the database and the revision
/// it holds, so a later step can refuse to use it after the source changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedBackup {
    pub path: PathBuf,
    pub database_id: String,
    pub sql_schema_version: u32,
    pub database_revision: u64,
    pub record_count: u64,
}

/// What the source held at the moment of the copy.
struct SourceSnapshot {
    database_revision: u64,
    record_count: u64,
}

fn record_count(connection: &Connection) -> Result<u64> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;
    Ok(count as u64)
}

fn prepare_exclusive_backup_file(backup_path: &Path) -> Result<()> {
    files::refuse_symbolic_link_components(backup_path, true)?;
    if let Some(parent) = backup_path.parent() {
        files::refuse_symbolic_link_components(parent, false)?;
    }
    match std::fs::symlink_metadata(backup_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Error::refused(format!(
                "Refusing to write a migration backup through a symbolic link: {}",
                backup_path.display()
            )));
        }
        Ok(_) => {
            return Err(Error::refused(format!(
                "Refusing to overwrite existing migration backup: {}",
                backup_path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    match files::create_new_private(backup_path) {
        Ok(file) => {
            drop(file);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(Error::refused(format!("Refusing to overwrite existing migration backup: {}", backup_path.display())))
        }
        Err(error) => Err(error.into()),
    }
}

/// Copy the source into the open destination from one source snapshot.
fn copy_snapshot(source: &Connection, destination: &mut Connection) -> Result<SourceSnapshot> {
    // A read transaction pins the snapshot. The backup step uses the open
    // transaction instead of starting its own, so the counts read here
    // describe exactly the pages it copies.
    let snapshot = rusqlite::Transaction::new_unchecked(source, rusqlite::TransactionBehavior::Deferred)?;
    let database_revision = super::database_revision(&snapshot)?;
    let record_count = record_count(&snapshot)?;
    {
        let backup = Backup::new(&snapshot, destination)
            .map_err(|error| Error::other(format!("SQLite backup failed: {error}")))?;
        match backup.step(-1).map_err(|error| Error::other(format!("SQLite backup failed: {error}")))? {
            rusqlite::backup::StepResult::Done => {}
            other => return Err(Error::other(format!("SQLite backup failed: the copy stopped early ({other:?})"))),
        }
    }
    snapshot.rollback()?;
    Ok(SourceSnapshot { database_revision, record_count })
}

fn verify(
    destination: &Connection,
    expected: &DatabaseMetadata,
    snapshot: &SourceSnapshot,
) -> Result<(DatabaseMetadata, u64)> {
    let mut statement = destination.prepare("PRAGMA integrity_check")?;
    let lines: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| Error::other(format!("Migration backup integrity check failed: {error}")))?
        .collect::<std::result::Result<_, _>>()
        .map_err(|error| Error::other(format!("Migration backup integrity check failed: {error}")))?;
    drop(statement);
    let integrity = lines.join("\n");
    if integrity.trim() != "ok" {
        return Err(Error::other(format!("Migration backup failed integrity check: {}", integrity.trim())));
    }

    let marker: Option<String> = destination
        .query_row("SELECT application_marker FROM database_metadata WHERE singleton_id = 1", [], |row| row.get(0))
        .optional()
        .map_err(|error| Error::other(format!("Could not read metadata from the migration backup: {error}")))?;
    match marker {
        None => return Err(Error::other("Migration backup is missing Athletic Standard metadata")),
        Some(marker) if marker != SQLITE_APPLICATION_MARKER => {
            return Err(Error::other("Migration backup has the wrong application marker"));
        }
        Some(_) => {}
    }
    let metadata = read_metadata(destination)
        .map_err(|error| Error::other(format!("Migration backup is missing Athletic Standard metadata: {error}")))?;
    if metadata.database_id != expected.database_id {
        return Err(Error::other("Migration backup database_id does not match the source database"));
    }
    if metadata.sql_schema_version != expected.sql_schema_version {
        return Err(Error::other("Migration backup sql_schema_version does not match the source database"));
    }
    let backup_revision = super::database_revision(destination)?;
    if backup_revision != snapshot.database_revision {
        return Err(Error::other(format!(
            "Migration backup database revision {backup_revision} does not match the source database {}",
            snapshot.database_revision
        )));
    }
    let backup_count = record_count(destination)
        .map_err(|error| Error::other(format!("Could not read record counts from the migration backup: {error}")))?;
    if backup_count != snapshot.record_count {
        return Err(Error::other(format!(
            "Migration backup record count {backup_count} does not match the source database {}",
            snapshot.record_count
        )));
    }
    Ok((metadata, backup_revision))
}

fn remove_failed_backup(backup_path: &Path) {
    // These files were created by this call. The source database's own
    // sidecars are never touched.
    for candidate in [backup_path.to_path_buf(), files::sidecar(backup_path, "-journal")] {
        let _ = std::fs::remove_file(candidate);
    }
}

/// Copy `store` to a new file at `backup_path` and verify the copy against
/// `expected` and the source snapshot. The file must not exist.
pub fn create_verified_backup(
    store: &Store,
    backup_path: &Path,
    expected: &DatabaseMetadata,
) -> Result<VerifiedBackup> {
    let source = store.connection();
    let backup_path = files::absolute(backup_path)?;
    prepare_exclusive_backup_file(&backup_path)?;
    let identity = files::identity(&backup_path)?;
    let result = (|| {
        let mut destination = Connection::open_with_flags(
            &backup_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        files::assert_same_file(&backup_path, identity)?;
        let snapshot = copy_snapshot(source, &mut destination)?;
        // One self-contained file: no WAL to carry next to the backup.
        let mode: String = destination.pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("delete") {
            return Err(Error::other(format!("Migration backup could not leave WAL mode: {mode}")));
        }
        let (metadata, database_revision) = verify(&destination, expected, &snapshot)?;
        destination.close().map_err(|(_, error)| Error::from(error))?;
        files::set_owner_only(&backup_path)?;
        if let Some(parent) = backup_path.parent() {
            files::sync_directory(parent)?;
        }
        Ok(VerifiedBackup {
            path: backup_path.clone(),
            database_id: metadata.database_id,
            sql_schema_version: metadata.sql_schema_version,
            database_revision,
            record_count: snapshot.record_count,
        })
    })();
    if result.is_err() && files::identity(&backup_path).ok() == Some(identity) {
        remove_failed_backup(&backup_path);
    }
    result
}

impl Store {
    /// Make a verified backup of this database at a new path.
    pub fn create_verified_backup(&self, backup_path: &Path) -> Result<VerifiedBackup> {
        let metadata = read_metadata(self.connection())?;
        create_verified_backup(self, backup_path, &metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_matches_the_old_format() {
        let millis = time::parse_utc_timestamp("2026-01-15T12:00:00.123Z").unwrap();
        assert_eq!(format_migration_backup_timestamp(millis), "20260115T120000.123Z");
    }

    #[test]
    fn names_planned_in_one_millisecond_differ() {
        let millis = time::parse_utc_timestamp("2026-01-15T12:00:00.123Z").unwrap();
        let first = default_migration_backup_path(Path::new("/tmp/athlete.ath.sqlite"), millis);
        let second = default_migration_backup_path(Path::new("/tmp/athlete.ath.sqlite"), millis);
        assert_ne!(first, second);
        let text = first.display().to_string();
        assert!(text.starts_with("/tmp/athlete.ath.sqlite.migration-backup.20260115T120000.123Z-"), "{text}");
        assert!(text.ends_with(".sqlite"));
        let repair = default_repair_backup_path(Path::new("/tmp/athlete.ath.sqlite"), millis);
        assert!(repair.display().to_string().contains(".repair-backup.20260115T120000.123Z-"));
    }
}
