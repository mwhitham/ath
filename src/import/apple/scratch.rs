//! A private temporary SQLite file for work the reader must finish later.
//!
//! Some Apple records cannot be sent to the plan when they are read. Samples
//! are listed by day and time, sleep fragments are joined into nights by
//! writer, and workouts wait for their route files. Holding those in memory
//! would grow with the export, so they wait here. The file lives in an
//! owner-only temporary directory this value created, and the directory is
//! removed when the value is dropped. It is working data, not a record.

use std::path::PathBuf;

use rusqlite::Connection;

use crate::error::Result;
use crate::files;

pub struct Scratch {
    directory: PathBuf,
    connection: Option<Connection>,
}

const SCHEMA: &str = "
CREATE TABLE samples (
  phase INTEGER NOT NULL,
  bucket INTEGER NOT NULL,
  day INTEGER NOT NULL,
  instant_ms INTEGER NOT NULL,
  seq INTEGER NOT NULL,
  source_id TEXT NOT NULL,
  manual INTEGER NOT NULL,
  quantity TEXT NOT NULL,
  at TEXT NOT NULL,
  value REAL NOT NULL,
  duration_ms INTEGER NOT NULL,
  input_raw TEXT,
  input_value REAL,
  input_unit TEXT,
  zone TEXT,
  PRIMARY KEY (phase, bucket, day, instant_ms, seq)
) WITHOUT ROWID;
CREATE TABLE fragments (
  grp INTEGER NOT NULL,
  start_ms INTEGER NOT NULL,
  seq INTEGER NOT NULL,
  body TEXT NOT NULL,
  PRIMARY KEY (grp, start_ms, seq)
) WITHOUT ROWID;
CREATE TABLE workouts (seq INTEGER PRIMARY KEY, body TEXT NOT NULL);
CREATE TABLE attached_routes (seq INTEGER PRIMARY KEY, workout INTEGER NOT NULL, body TEXT NOT NULL);
CREATE INDEX attached_routes_by_workout ON attached_routes (workout, seq);
CREATE TABLE route_texts (name TEXT PRIMARY KEY, body TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE derived (seq INTEGER PRIMARY KEY, body TEXT NOT NULL);
";

impl Scratch {
    pub fn create() -> Result<Scratch> {
        let directory = files::make_temporary_directory("ath-apple-")?;
        let result = (|| {
            let path = directory.join("scratch.sqlite");
            drop(files::create_new_private(&path)?);
            let connection = Connection::open(&path)?;
            connection.execute_batch(
                "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA temp_store = FILE; PRAGMA cache_size = -8192;",
            )?;
            connection.execute_batch(SCHEMA)?;
            connection.execute_batch("BEGIN")?;
            Ok(connection)
        })();
        match result {
            Ok(connection) => Ok(Scratch { directory, connection: Some(connection) }),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&directory);
                Err(error)
            }
        }
    }

    pub fn connection(&self) -> &Connection {
        self.connection.as_ref().expect("scratch connection is open until drop")
    }

    /// End the write transaction, so later reads see a settled file.
    pub fn settle(&self) -> Result<()> {
        if !self.connection().is_autocommit() {
            self.connection().execute_batch("COMMIT")?;
        }
        Ok(())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = connection.close();
        }
        // Only the directory this value created is removed.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
