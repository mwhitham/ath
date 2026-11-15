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

/// How much a restore grows the database file at a time. Growing a large
/// file 4 KiB page by page left the 3-million-record restore waiting on the
/// file system; 8 MiB at a time removed most of that wait. The file can end
/// less than one chunk longer than the database: zeros past the last page,
/// which SQLite does not read. Later writes use that space first, and
/// `VACUUM` removes it.
pub const RESTORE_FILE_CHUNK_BYTES: i32 = 8 * 1024 * 1024;

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

    /// Switch a read-write connection to WAL now instead of at the first
    /// write. The JavaScript `openDatabase` did this when it opened a file.
    pub fn enable_wal(&self) -> Result<()> {
        if self.read_only {
            return Err(Error::refused("This database was opened read-only"));
        }
        enable_wal(&self.connection)
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
        let result = self.write_transaction(operation);
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

    /// `bulk_transaction` for a restore into a new, empty database. The
    /// transaction uses a rollback journal instead of the write-ahead log, so
    /// each page is written to the database file once. In WAL mode every page
    /// went to the log and then again to the file at the checkpoint. Pages
    /// added to an empty file need no copy in the journal, so the journal
    /// stays small. The file grows `RESTORE_FILE_CHUNK_BYTES` at a time.
    ///
    /// The change is still one atomic transaction. A failure rolls it back,
    /// and a process stopped part way leaves a journal that the next writable
    /// open rolls back, so the database is empty again. The file returns to
    /// WAL mode before this returns. When SQLite cannot leave WAL mode, for
    /// example because another connection has the file open, the restore
    /// runs in WAL mode, as `bulk_transaction`.
    ///
    /// The caller must check, inside the transaction, that the database is
    /// still the empty one it previewed.
    pub fn fresh_restore_transaction<T>(
        &mut self,
        operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        if self.read_only {
            return Err(Error::refused("This database was opened read-only"));
        }
        let journal_before: String = self.connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if !journal_before.eq_ignore_ascii_case("wal") {
            return self.bulk_transaction(operation);
        }
        let switched: std::result::Result<String, _> =
            self.connection.pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0));
        if !switched.is_ok_and(|mode| mode.eq_ignore_ascii_case("delete")) {
            return self.bulk_transaction(operation);
        }
        let previous: Result<i64> =
            self.connection.pragma_query_value(None, "temp_store", |row| row.get(0)).map_err(Error::from);
        let result = previous.and_then(|previous| {
            self.connection.pragma_update(None, "temp_store", "MEMORY")?;
            self.set_file_chunk(RESTORE_FILE_CHUNK_BYTES);
            let result = self.write_transaction(operation);
            self.set_file_chunk(0);
            let restored = self.connection.pragma_update(None, "temp_store", previous);
            let value = result?;
            restored?;
            Ok(value)
        });
        // Back to WAL mode whatever happened. The journal is gone by now:
        // a commit or a rollback removed it.
        let wal = enable_wal(&self.connection);
        let value = result?;
        if let Err(error) = wal {
            return Err(Error::other(format!(
                "The change was committed, but the file could not return to WAL mode: {error}. The next write returns it to WAL mode."
            )));
        }
        files::set_owner_only_database(&self.path)?;
        Ok(value)
    }

    /// Grow the database file `bytes` at a time; 0 grows it page by page, as
    /// SQLite does by default. A chunk is allocated when the first page in it
    /// is written. The setting belongs to this connection's open file. A
    /// failure only leaves the default in place, so it is not an error.
    fn set_file_chunk(&self, bytes: i32) {
        let mut size: std::ffi::c_int = bytes;
        // SAFETY: the handle belongs to this open connection, "main" is a
        // NUL-terminated name, and SQLite reads one C int through the pointer
        // during the call.
        unsafe {
            rusqlite::ffi::sqlite3_file_control(
                self.connection.handle(),
                c"main".as_ptr(),
                rusqlite::ffi::SQLITE_FCNTL_CHUNK_SIZE,
                (&mut size as *mut std::ffi::c_int).cast(),
            );
        }
    }

    /// One write transaction with foreign keys deferred to commit, so related
    /// rows can be written in any order.
    fn write_transaction<T>(&mut self, operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let enabled: i64 = tx.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
        if enabled != 1 {
            return Err(Error::other("SQLite foreign-key enforcement is disabled in write transaction"));
        }
        tx.pragma_update(None, "defer_foreign_keys", "ON")?;
        let value = operation(&tx)?;
        tx.commit()?;
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

    fn journal_mode(connection: &Connection) -> String {
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0)).unwrap()
    }

    fn mode_inside(tx: &rusqlite::Transaction<'_>) -> Result<String> {
        Ok(tx.pragma_query_value(None, "journal_mode", |row| row.get(0))?)
    }

    #[test]
    fn a_fresh_restore_uses_a_rollback_journal_and_ends_in_wal_mode() {
        let directory = files::make_temporary_directory("ath-fresh-restore-").unwrap();
        let path = directory.join("fresh.ath.sqlite");
        let mut store = Store::create(&path, CreateOptions::default()).unwrap();
        assert_eq!(journal_mode(store.connection()), "wal");
        let inside = store
            .fresh_restore_transaction(|tx| {
                tx.execute("UPDATE database_state SET current_revision = 3 WHERE singleton_id = 1", [])?;
                mode_inside(tx)
            })
            .unwrap();
        assert_eq!(inside, "delete");
        assert_eq!(journal_mode(store.connection()), "wal");
        assert_eq!(temp_store(&store), 0);
        assert_eq!(store.database_revision().unwrap(), 3);
        assert!(!files::sidecar(&path, "-journal").exists());
        store.close().unwrap();

        // A new connection reads the committed change and finds WAL mode.
        let reopened = Store::open(&path, OpenOptions::default()).unwrap();
        assert_eq!(reopened.database_revision().unwrap(), 3);
        assert_eq!(journal_mode(reopened.connection()), "wal");
        drop(reopened);
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn database_bytes(store: &Store) -> u64 {
        let pages: i64 = store.connection().pragma_query_value(None, "page_count", |row| row.get(0)).unwrap();
        let size: i64 = store.connection().pragma_query_value(None, "page_size", |row| row.get(0)).unwrap();
        (pages * size) as u64
    }

    /// The file grows a chunk at a time during a restore. It ends less than
    /// one chunk longer than the database, and later writes and checks treat
    /// it as any other database.
    #[test]
    fn a_fresh_restore_grows_the_file_in_chunks() {
        let directory = files::make_temporary_directory("ath-fresh-restore-chunk-").unwrap();
        let path = directory.join("fresh.ath.sqlite");
        let mut store = Store::create(&path, CreateOptions::default()).unwrap();
        store
            .fresh_restore_transaction(|tx| {
                tx.execute_batch(
                    "CREATE TEMP TABLE numbers(n);
                     WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence WHERE n < 300)
                     INSERT INTO numbers SELECT n FROM sequence;
                     CREATE TABLE filler(data BLOB);
                     INSERT INTO filler SELECT zeroblob(10000) FROM numbers;",
                )?;
                Ok(())
            })
            .unwrap();
        let file = std::fs::metadata(&path).unwrap().len();
        let database = database_bytes(&store);
        assert!(database > 3_000_000, "{database}");
        assert_eq!(file % RESTORE_FILE_CHUNK_BYTES as u64, 0, "{file}");
        assert!(file >= database && file - database < RESTORE_FILE_CHUNK_BYTES as u64, "{file} {database}");

        store.transaction(|tx| Ok(tx.execute("UPDATE database_state SET current_revision = 1", [])?)).unwrap();
        let integrity: String = store.connection().query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
        assert_eq!(integrity, "ok");
        assert_eq!(journal_mode(store.connection()), "wal");
        store.close().unwrap();
        assert_eq!(Store::open(&path, OpenOptions::default()).unwrap().database_revision().unwrap(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_failed_fresh_restore_leaves_the_empty_database_as_it_was() {
        let directory = files::make_temporary_directory("ath-fresh-restore-fail-").unwrap();
        let path = directory.join("fresh.ath.sqlite");
        let mut store = Store::create(&path, CreateOptions::default()).unwrap();
        let before = read_metadata(store.connection()).unwrap();
        let failed = store.fresh_restore_transaction(|tx| {
            tx.execute("UPDATE database_state SET current_revision = 9 WHERE singleton_id = 1", [])?;
            tx.execute(
                "UPDATE database_metadata SET database_id = ?1 WHERE singleton_id = 1",
                [crate::ids::new_record_id()],
            )?;
            Err::<(), _>(Error::other("stop"))
        });
        assert!(failed.is_err());
        assert_eq!(store.database_revision().unwrap(), 0);
        assert_eq!(read_metadata(store.connection()).unwrap(), before);
        assert_eq!(journal_mode(store.connection()), "wal");
        assert_eq!(temp_store(&store), 0);
        assert!(!files::sidecar(&path, "-journal").exists());
        store.close().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_fresh_restore_keeps_wal_mode_while_another_connection_has_the_file_open() {
        let directory = files::make_temporary_directory("ath-fresh-restore-shared-").unwrap();
        let path = directory.join("fresh.ath.sqlite");
        let mut store = Store::create(&path, CreateOptions::default()).unwrap();
        store.connection().busy_timeout(Duration::from_millis(50)).unwrap();
        let other = Connection::open(&path).unwrap();
        let _: i64 = other.query_row("SELECT current_revision FROM database_state", [], |row| row.get(0)).unwrap();
        let inside = store
            .fresh_restore_transaction(|tx| {
                tx.execute("UPDATE database_state SET current_revision = 2 WHERE singleton_id = 1", [])?;
                mode_inside(tx)
            })
            .unwrap();
        assert_eq!(inside, "wal");
        assert_eq!(journal_mode(store.connection()), "wal");
        let seen: i64 = other.query_row("SELECT current_revision FROM database_state", [], |row| row.get(0)).unwrap();
        assert_eq!(seen, 2);
        drop(other);
        store.close().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
