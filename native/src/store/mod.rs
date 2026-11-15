//! The SQLite store: connections, transactions, reads, and writes.
//!
//! A `Store` owns one connection to one local database file. Each write runs as
//! one transaction that covers the complete logical change, including its
//! revisions, typed rows, child rows, import keys, and the database revision.
//! Reading commands open the file read-only: they do not change the journal
//! mode, the file header, or athlete data, and they do not checkpoint on close.

pub mod backup;
pub mod check;
pub mod contract;
pub mod encode;
pub mod migrate;
pub mod pending;
pub mod query;
pub mod read;
pub mod repair;
pub mod stats;
pub mod write;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::files;
use crate::model::{self, time};

pub use encode::Layout;
pub use migrate::{MigrationPreview, SQL_SCHEMA_VERSION};
pub use write::{Applied, Changes, Correction, Deletion, ExtensionsUpdate, NewRecord};

/// `ATH1` as a SQLite application ID.
pub const SQLITE_APPLICATION_ID: i32 = 0x4154_4831;
pub const SQLITE_APPLICATION_MARKER: &str = "ATH1";
pub const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

/// Database versions this software reads.
pub const READABLE_DATABASE_VERSIONS: &[&str] = &["1.0.0", "2.0.0", "2.1.0", "2.1.1"];

/// The default database file name in a working directory.
pub const DEFAULT_FILENAME: &str = "athlete.ath.sqlite";

/// The identity and versions stored in `database_metadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabaseMetadata {
    pub application_marker: String,
    pub sql_schema_version: u32,
    pub athleticstandard_version: String,
    pub database_id: String,
    pub created_at: String,
}

/// How to open a database.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// Open read-only. Read-only opens never change file state.
    pub read_only: bool,
    /// Allow a database that still needs `ath migrate`. Used by migrate and repair.
    pub allow_outdated_schema: bool,
}

/// Options for a new database.
#[derive(Debug, Clone, Default)]
pub struct CreateOptions {
    pub database_id: Option<String>,
    pub created_at: Option<String>,
}

pub struct Store {
    connection: Connection,
    path: PathBuf,
    metadata: DatabaseMetadata,
    read_only: bool,
    /// Warnings found while opening, such as files other users can read.
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("path", &self.path).field("metadata", &self.metadata).finish()
    }
}

/// Prepared statements kept per connection. The encoder and the check use
/// one statement per typed table and child table, about 70 in all. The
/// default of 16 made SQLite prepare most of them again for every record.
pub const PREPARED_STATEMENT_CACHE: usize = 256;

fn configure(connection: &Connection) -> Result<()> {
    connection.set_prepared_statement_cache_capacity(PREPARED_STATEMENT_CACHE);
    connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let enabled: i64 = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if enabled != 1 {
        return Err(Error::other("SQLite foreign-key enforcement could not be enabled"));
    }
    Ok(())
}

/// SQLite could not open the file, or could not create the files a WAL
/// database needs beside it.
fn cannot_open_wal(failure: &rusqlite::ffi::Error) -> bool {
    failure.code == rusqlite::ErrorCode::CannotOpen
        || failure.extended_code == rusqlite::ffi::SQLITE_READONLY_CANTINIT
        || failure.extended_code == rusqlite::ffi::SQLITE_READONLY_DIRECTORY
}

/// Read and check `database_metadata`.
pub fn read_metadata(connection: &Connection) -> Result<DatabaseMetadata> {
    let mut statement = connection
        .prepare(
            "SELECT application_marker, sql_schema_version, athleticstandard_version, database_id, created_at
             FROM database_metadata WHERE singleton_id = 1",
        )
        .map_err(|error| match error {
            // The file could not be opened at all; that is not a missing table.
            rusqlite::Error::SqliteFailure(failure, _) if cannot_open_wal(&failure) => Error::Database(error),
            _ => Error::refused("File is not an Athletic Standard database: metadata table is missing"),
        })?;
    let rows: Vec<(
        rusqlite::types::Value,
        rusqlite::types::Value,
        rusqlite::types::Value,
        rusqlite::types::Value,
        rusqlite::types::Value,
    )> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
        .collect::<std::result::Result<_, _>>()?;
    if rows.len() != 1 {
        return Err(Error::refused("Athletic Standard database metadata is missing or duplicated"));
    }
    let (marker, sql_version, version, database_id, created_at) = rows.into_iter().next().expect("one row");
    let text = |value: rusqlite::types::Value, name: &str| match value {
        rusqlite::types::Value::Text(text) => Ok(text),
        _ => Err(Error::refused(format!("Database metadata {name} is not text"))),
    };
    let marker = text(marker, "application_marker")?;
    if marker != SQLITE_APPLICATION_MARKER {
        return Err(Error::refused(format!("Unsupported database application marker: {marker}")));
    }
    let version = text(version, "athleticstandard_version")?;
    if !READABLE_DATABASE_VERSIONS.contains(&version.as_str()) {
        return Err(Error::refused(format!("Unsupported Athletic Standard database version: {version}")));
    }
    let sql_schema_version = match sql_version {
        rusqlite::types::Value::Integer(number) if number >= 0 => number as u32,
        rusqlite::types::Value::Real(number) if number >= 0.0 => number as u32,
        _ => return Err(Error::refused("Database metadata sql_schema_version is not numeric")),
    };
    let database_id = text(database_id, "database_id")?;
    if !model::is_record_id(&database_id) {
        return Err(Error::refused("Database metadata database_id is not a lowercase UUIDv7"));
    }
    let created_at = text(created_at, "created_at")?;
    if !time::is_utc_timestamp(&created_at) {
        return Err(Error::refused("Database metadata created_at is not a UTC timestamp"));
    }
    Ok(DatabaseMetadata {
        application_marker: marker,
        sql_schema_version,
        athleticstandard_version: version,
        database_id,
        created_at,
    })
}

fn check_header(connection: &Connection, metadata: &DatabaseMetadata) -> Result<()> {
    let application_id: i32 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if application_id != 0 && application_id != SQLITE_APPLICATION_ID {
        return Err(Error::refused(format!(
            "SQLite application ID {application_id} does not belong to Athletic Standard"
        )));
    }
    if metadata.sql_schema_version > SQL_SCHEMA_VERSION {
        return Err(Error::refused(format!(
            "Database SQL schema {} is newer than supported schema {SQL_SCHEMA_VERSION}",
            metadata.sql_schema_version
        )));
    }
    let user_version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version != metadata.sql_schema_version {
        return Err(Error::refused(format!(
            "SQLite user_version {user_version} does not match database metadata {}",
            metadata.sql_schema_version
        )));
    }
    Ok(())
}

/// The default backup path for a migration: next to the database, named with
/// the time it was planned. The name format is the 2.1.0 format.
pub fn default_migration_backup_path(path: &Path, now_millis: i64) -> PathBuf {
    backup::default_migration_backup_path(path, now_millis)
}

impl Store {
    /// Open an existing database. Read-only opens use a read-only connection.
    pub fn open(path: impl AsRef<Path>, options: OpenOptions) -> Result<Store> {
        let path = files::absolute(path.as_ref())?;
        files::refuse_symbolic_link_components(&path, false)?;
        files::assert_parent_not_writable_by_others(&path)?;
        let warnings = files::readable_path_warnings(&path);
        let identity = files::identity(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Error::not_found(format!("No database file at {}", path.display())),
            _ => error.into(),
        })?;
        let flags = if options.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let connection = Connection::open_with_flags(&path, flags)?;
        files::assert_same_file(&path, identity)?;
        configure(&connection)?;
        let metadata = read_metadata(&connection).map_err(|error| match error {
            Error::Database(rusqlite::Error::SqliteFailure(failure, _)) if options.read_only && cannot_open_wal(&failure) => {
                Error::refused(format!(
                    "Cannot read {} read-only. A WAL database needs its -wal and -shm files, and this folder does not allow creating them. Nothing was changed.",
                    path.display()
                ))
            }
            other => other,
        })?;
        check_header(&connection, &metadata)?;
        if metadata.sql_schema_version < SQL_SCHEMA_VERSION && !options.allow_outdated_schema {
            let backup = default_migration_backup_path(&path, time::now_millis());
            return Err(Error::MigrationRequired(Box::new(migrate::preview(
                metadata.sql_schema_version,
                &metadata.database_id,
                Some(backup.display().to_string()),
            ))));
        }
        Ok(Store { connection, path, metadata, read_only: options.read_only, warnings })
    }

    /// Create a new database file. The file must not exist. A failed creation
    /// removes only the file this call created.
    pub fn create(path: impl AsRef<Path>, options: CreateOptions) -> Result<Store> {
        let path = files::absolute(path.as_ref())?;
        let database_id = options.database_id.unwrap_or_else(crate::ids::new_record_id);
        if !model::is_record_id(&database_id) {
            return Err(Error::other("database_id must be a lowercase UUIDv7"));
        }
        let created_at = options.created_at.unwrap_or_else(time::now_utc);
        if !time::is_utc_timestamp(&created_at) {
            return Err(Error::other("created_at must be a UTC timestamp with milliseconds"));
        }
        files::refuse_symbolic_link_components(&path, true)?;
        let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
        files::create_private_dir_all(&parent)?;
        files::refuse_symbolic_link_components(&parent, false)?;
        files::assert_parent_not_writable_by_others(&path)?;
        match files::create_new_private(&path) {
            Ok(file) => drop(file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Error::refused(format!("Refusing to replace existing database: {}", path.display())));
            }
            Err(error) => return Err(error.into()),
        }
        let identity = files::identity(&path)?;
        let result = (|| {
            let connection = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            files::assert_same_file(&path, identity)?;
            configure(&connection)?;
            enable_wal(&connection)?;
            install_schema(&connection, &database_id, &created_at)?;
            files::set_owner_only_database(&path)?;
            let metadata = read_metadata(&connection)?;
            Ok(Store { connection, path: path.clone(), metadata, read_only: false, warnings: Vec::new() })
        })();
        if result.is_err() {
            remove_partial_database(&path, identity);
        }
        result
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> &DatabaseMetadata {
        &self.metadata
    }

    pub fn layout(&self) -> Layout {
        Layout { sql_schema_version: self.metadata.sql_schema_version }
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// The connection, for read queries in other library modules.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// The monotonic database revision.
    pub fn database_revision(&self) -> Result<u64> {
        database_revision(&self.connection)
    }

    /// Re-read metadata after a migration.
    pub fn reload_metadata(&mut self) -> Result<()> {
        self.metadata = read_metadata(&self.connection)?;
        Ok(())
    }

    /// Run a complete logical change in one write transaction. Foreign keys
    /// are deferred to commit, so related rows can be written in any order.
    pub fn transaction<T>(&mut self, operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>) -> Result<T> {
        if self.read_only {
            return Err(Error::refused("This database was opened read-only"));
        }
        let journal_before: String = self.connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        enable_wal(&self.connection)?;
        let result = (|| {
            let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let enabled: i64 = tx.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
            if enabled != 1 {
                return Err(Error::other("SQLite foreign-key enforcement is disabled in write transaction"));
            }
            tx.pragma_update(None, "defer_foreign_keys", "ON")?;
            let value = operation(&tx)?;
            tx.commit()?;
            Ok(value)
        })();
        if result.is_err() && !journal_before.eq_ignore_ascii_case("wal") {
            // Return a file that started in rollback-journal mode to that mode.
            let _ = self.connection.pragma_update(None, "journal_mode", "DELETE");
        }
        if result.is_ok() {
            files::set_owner_only_database(&self.path)?;
        }
        result
    }

    /// `transaction` for one large change made of many small statements,
    /// such as a package restore. SQLite keeps each statement's journal in
    /// memory rather than in a temporary file. A statement journal holds only
    /// the pages that one statement changed, and SQLite empties it when the
    /// statement ends, so memory stays small. In a file, each inserted row
    /// cost several write calls. The earlier setting returns afterwards.
    pub fn bulk_transaction<T>(
        &mut self,
        operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let previous: i64 = self.connection.pragma_query_value(None, "temp_store", |row| row.get(0))?;
        self.connection.pragma_update(None, "temp_store", "MEMORY")?;
        let result = self.transaction(operation);
        let restored = self.connection.pragma_update(None, "temp_store", previous);
        let value = result?;
        restored?;
        Ok(value)
    }

    /// Checkpoint and truncate the WAL after writes, then close.
    pub fn close(self) -> Result<()> {
        if !self.read_only {
            let _ = self.connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        }
        self.connection.close().map_err(|(_, error)| error.into())
    }
}

pub fn database_revision(connection: &Connection) -> Result<u64> {
    let revision: Option<i64> = connection
        .query_row("SELECT current_revision FROM database_state WHERE singleton_id = 1", [], |row| row.get(0))
        .optional()?;
    revision.map(|value| value as u64).ok_or_else(|| Error::other("Database revision is missing"))
}

fn enable_wal(connection: &Connection) -> Result<()> {
    let mode: String = connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::other(format!("SQLite did not enable WAL mode: {mode}")));
    }
    Ok(())
}

/// Install the schema on a new file: the ordered migration chain, metadata,
/// the database revision, and the file header.
fn install_schema(connection: &Connection, database_id: &str, created_at: &str) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let first = &migrate::MIGRATION_CHAIN[0];
    tx.execute_batch(first.sql)?;
    tx.execute(
        "INSERT INTO database_metadata (singleton_id, application_marker, sql_schema_version, athleticstandard_version, database_id, created_at)
         VALUES (1, ?1, 1, '1.0.0', ?2, ?3)",
        rusqlite::params![SQLITE_APPLICATION_MARKER, database_id, created_at],
    )?;
    tx.execute("INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0)", [])?;
    for step in &migrate::MIGRATION_CHAIN[1..] {
        tx.execute_batch(step.sql)?;
    }
    tx.pragma_update(None, "application_id", SQLITE_APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", SQL_SCHEMA_VERSION)?;
    let metadata = read_metadata(&tx)?;
    if metadata.sql_schema_version != SQL_SCHEMA_VERSION
        || metadata.athleticstandard_version != model::ATHLETIC_STANDARD_VERSION
    {
        return Err(Error::other("The migration chain did not reach the current schema"));
    }
    tx.commit()?;
    Ok(())
}

fn remove_partial_database(path: &Path, identity: files::FileIdentity) {
    if files::identity(path).ok() != Some(identity) {
        return;
    }
    for candidate in [path.to_path_buf(), files::sidecar(path, "-wal"), files::sidecar(path, "-shm")] {
        let _ = std::fs::remove_file(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(store: &Store) -> i64 {
        store.connection().pragma_query_value(None, "temp_store", |row| row.get(0)).unwrap()
    }

    #[test]
    fn a_bulk_transaction_keeps_statement_journals_in_memory_and_restores_the_setting() {
        let directory = files::make_temporary_directory("ath-bulk-transaction-").unwrap();
        let mut store = Store::create(directory.join("bulk.ath.sqlite"), CreateOptions::default()).unwrap();
        assert_eq!(temp_store(&store), 0);
        let inside = store
            .bulk_transaction(|tx| Ok(tx.pragma_query_value(None, "temp_store", |row| row.get::<_, i64>(0))?))
            .unwrap();
        assert_eq!(inside, 2);
        assert_eq!(temp_store(&store), 0);

        // A failed change rolls back and still restores the setting.
        let failed = store.bulk_transaction(|tx| {
            tx.execute("UPDATE database_state SET current_revision = 7 WHERE singleton_id = 1", [])?;
            Err::<(), _>(Error::other("stop"))
        });
        assert!(failed.is_err());
        assert_eq!(temp_store(&store), 0);
        assert_eq!(store.database_revision().unwrap(), 0);
        store.close().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
