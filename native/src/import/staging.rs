//! The staged import plan: a private temporary SQLite file.
//!
//! Candidates stream into this file as a reader finds them, so a large export
//! is never held in memory. Matching writes its decisions into the same rows.
//! The preview lists and the final write read them back in plan order.
//!
//! The file is disposable working data, not a second athlete record. It lives
//! in an owner-only temporary directory that this value created, and it is
//! removed when the value is dropped. Only that directory is ever removed.

use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::files;
use crate::json::{self, Value};

use super::candidate::{Candidate, KeyPart, MatchKey};

/// What matching decided for one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    New,
    Matching,
    Changed,
    /// Left out of the plan and reported as a conflict.
    Conflict,
    /// A keyed route's repeat of an earlier candidate with the same key and
    /// the same data. It names the earlier candidate's record and writes nothing.
    Repeat,
    /// A keyed route's candidate whose stored record or source is deleted. It
    /// is reported as left out and writes nothing.
    LeftOut,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::New => "new",
            Action::Matching => "matching",
            Action::Changed => "changed",
            Action::Conflict => "conflict",
            Action::Repeat => "repeat",
            Action::LeftOut => "left_out",
        }
    }

    fn parse(text: &str) -> Option<Action> {
        Some(match text {
            "new" => Action::New,
            "matching" => Action::Matching,
            "changed" => Action::Changed,
            "conflict" => Action::Conflict,
            "repeat" => Action::Repeat,
            "left_out" => Action::LeftOut,
            _ => return None,
        })
    }
}

/// One staged candidate as matching sees it.
#[derive(Debug, Clone)]
pub(crate) struct StagedRow {
    pub position: i64,
    pub section: i64,
    pub record_id: String,
    pub record_type: String,
    pub data_json: String,
    pub source_id: Option<String>,
    pub key_parts: String,
    pub numbered: bool,
    pub ordinal: Option<i64>,
    pub existing_record_id: Option<String>,
    /// The reader's group, for a keyed route: its order among left-out lines,
    /// and the label those lines start with.
    pub group_seq: Option<i64>,
    pub group_label: Option<String>,
}

/// A keyed route's group: one source object, file, or row whose records a
/// left-out line names together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    pub seq: i64,
    pub label: String,
}

/// One planned change as the preview and the write see it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRecord {
    pub record_id: String,
    pub record_type: String,
    pub action: Action,
    /// The current revision of an existing record; absent for a new one.
    pub current_revision: Option<u64>,
    pub match_key: String,
    /// The complete record data that would be stored.
    pub data_json: String,
}

/// What matching decided for one staged row.
pub(crate) struct Decision<'a> {
    /// The new record's ID, or the stored record it matched.
    pub record_id: &'a str,
    /// The complete data with the resolved source ID.
    pub data_json: &'a str,
    pub match_key: &'a str,
    /// The key without its number, for numbered keys that stay in the plan.
    pub collision_group: Option<&'a str>,
    pub action: Action,
    pub current_revision: Option<u64>,
}

/// A position in plan order: section, then arrival position.
pub(crate) type PlanCursor = Option<(i64, i64)>;

pub(crate) struct Staging {
    directory: PathBuf,
    connection: Option<Connection>,
    next_position: i64,
}

impl std::fmt::Debug for Staging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Staging").field("directory", &self.directory).finish()
    }
}

const SCHEMA: &str = "
CREATE TABLE candidates (
  position INTEGER PRIMARY KEY,
  section INTEGER NOT NULL,
  record_id TEXT NOT NULL,
  record_type TEXT NOT NULL,
  data_json TEXT NOT NULL,
  source_id TEXT,
  key_parts TEXT NOT NULL,
  numbered INTEGER NOT NULL,
  ordinal INTEGER,
  existing_record_id TEXT,
  route_points INTEGER NOT NULL,
  group_seq INTEGER,
  group_label TEXT,
  match_key TEXT,
  collision_group TEXT,
  action TEXT,
  current_revision INTEGER
);
CREATE UNIQUE INDEX candidates_plan_order ON candidates (section, position);
CREATE TABLE stems (stem TEXT PRIMARY KEY, count INTEGER NOT NULL) WITHOUT ROWID;
CREATE TABLE seen_keys (match_key TEXT PRIMARY KEY, position INTEGER NOT NULL) WITHOUT ROWID;
CREATE TABLE conflicts (
  conflict_order INTEGER PRIMARY KEY,
  match_key TEXT NOT NULL,
  record_id TEXT,
  message TEXT NOT NULL
);
";

const ROW_COLUMNS: &str =
    "position, section, record_id, record_type, data_json, source_id, key_parts, numbered, ordinal, \
     existing_record_id, group_seq, group_label";

fn staged_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StagedRow> {
    Ok(StagedRow {
        position: row.get(0)?,
        section: row.get(1)?,
        record_id: row.get(2)?,
        record_type: row.get(3)?,
        data_json: row.get(4)?,
        source_id: row.get(5)?,
        key_parts: row.get(6)?,
        numbered: row.get::<_, i64>(7)? != 0,
        ordinal: row.get(8)?,
        existing_record_id: row.get(9)?,
        group_seq: row.get(10)?,
        group_label: row.get(11)?,
    })
}

/// The JSON array of key parts; `null` marks the source ID. A literal key is
/// a JSON string.
fn key_parts_json(key: &MatchKey) -> String {
    if key.literal {
        let text = match key.parts.first() {
            Some(KeyPart::Text(text)) => text.clone(),
            _ => String::new(),
        };
        return json::to_string(&Value::String(text));
    }
    let parts = &key.parts;
    json::to_string(&Value::Array(
        parts
            .iter()
            .map(|part| match part {
                KeyPart::Text(text) => Value::String(text.clone()),
                KeyPart::Source => Value::Null,
            })
            .collect(),
    ))
}

impl Staging {
    pub fn create() -> Result<Staging> {
        let directory = files::make_temporary_directory("ath-import-")?;
        let result = (|| {
            let path = directory.join("plan.sqlite");
            drop(files::create_new_private(&path)?);
            let connection = Connection::open(&path)?;
            connection.execute_batch(
                "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA temp_store = FILE; PRAGMA cache_size = -8192;",
            )?;
            connection.execute_batch(SCHEMA)?;
            Ok(connection)
        })();
        match result {
            Ok(connection) => Ok(Staging { directory, connection: Some(connection), next_position: 0 }),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&directory);
                Err(error)
            }
        }
    }

    pub fn connection(&self) -> &Connection {
        self.connection.as_ref().expect("staging connection is open until drop")
    }

    /// Begin a write batch. Staging writes are grouped so a large export is
    /// not written one SQLite transaction per row.
    pub fn begin(&self) -> Result<()> {
        self.connection().execute_batch("BEGIN")?;
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        self.connection().execute_batch("COMMIT")?;
        Ok(())
    }

    /// Stage one candidate with its new record ID. Returns its ordinal in its
    /// collision group, for numbered keys.
    pub fn insert(
        &mut self,
        section: i64,
        record_id: &str,
        candidate_data: &str,
        candidate: &Candidate,
        existing_record_id: Option<&str>,
        group: Option<&Group>,
    ) -> Result<()> {
        let key_parts = key_parts_json(&candidate.key);
        let ordinal: Option<i64> = if candidate.key.numbered {
            Some(
                self.connection()
                    .prepare_cached(
                        "INSERT INTO stems (stem, count) VALUES (?1, 1)
                         ON CONFLICT (stem) DO UPDATE SET count = count + 1
                         RETURNING count - 1",
                    )?
                    .query_row([&key_parts], |row| row.get(0))?,
            )
        } else {
            None
        };
        let position = self.next_position;
        self.next_position += 1;
        self.connection()
            .prepare_cached(
                "INSERT INTO candidates (position, section, record_id, record_type, data_json, source_id, key_parts,
                    numbered, ordinal, existing_record_id, route_points, group_seq, group_label)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?
            .execute(params![
                position,
                section,
                record_id,
                candidate.data.record_type().as_str(),
                candidate_data,
                candidate.data.source_id(),
                key_parts,
                i64::from(candidate.key.numbered),
                ordinal,
                existing_record_id,
                candidate.route_points() as i64,
                group.map(|group| group.seq),
                group.map(|group| group.label.as_str()),
            ])?;
        Ok(())
    }

    /// Up to `limit` staged rows after `after` in plan order.
    pub fn rows_after(&self, after: PlanCursor, limit: usize) -> Result<Vec<StagedRow>> {
        let (section, position) = after.unwrap_or((-1, -1));
        let mut statement = self.connection().prepare_cached(&format!(
            "SELECT {ROW_COLUMNS} FROM candidates WHERE (section, position) > (?1, ?2)
             ORDER BY section, position LIMIT ?3"
        ))?;
        let rows = statement
            .query_map(params![section, position, limit as i64], staged_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Record matching's decision for one row.
    pub fn decide(&self, position: i64, decision: &Decision<'_>) -> Result<()> {
        self.connection()
            .prepare_cached(
                "UPDATE candidates SET record_id = ?2, data_json = ?3, match_key = ?4, collision_group = ?5,
                    action = ?6, current_revision = ?7
                 WHERE position = ?1",
            )?
            .execute(params![
                position,
                decision.record_id,
                decision.data_json,
                decision.match_key,
                decision.collision_group,
                decision.action.as_str(),
                decision.current_revision.map(|revision| revision as i64)
            ])?;
        Ok(())
    }

    /// The data, decided record ID, and action of the first staged row with
    /// this key, or record this row as the first.
    pub fn first_with_key(&self, match_key: &str, position: i64) -> Result<Option<(String, String, Option<Action>)>> {
        let inserted = self
            .connection()
            .prepare_cached("INSERT OR IGNORE INTO seen_keys (match_key, position) VALUES (?1, ?2)")?
            .execute(params![match_key, position])?;
        if inserted == 1 {
            return Ok(None);
        }
        Ok(self
            .connection()
            .prepare_cached(
                "SELECT candidates.data_json, candidates.record_id, candidates.action
                   FROM seen_keys JOIN candidates USING (position) WHERE seen_keys.match_key = ?1",
            )?
            .query_row([match_key], |row| {
                let action: Option<String> = row.get(2)?;
                Ok((row.get(0)?, row.get(1)?, action.as_deref().and_then(Action::parse)))
            })
            .optional()?)
    }

    pub fn add_conflict(&self, match_key: &str, record_id: Option<&str>, message: &str) -> Result<()> {
        self.connection()
            .prepare_cached("INSERT INTO conflicts (match_key, record_id, message) VALUES (?1, ?2, ?3)")?
            .execute(params![match_key, record_id, message])?;
        Ok(())
    }

    /// Collision groups of two or more planned records where one changed, by
    /// the plan position of each group's first record.
    pub fn changed_collision_groups(&self) -> Result<Vec<String>> {
        let mut statement = self.connection().prepare(
            "SELECT first.match_key
               FROM (SELECT collision_group, MIN(section * 9007199254740991 + position) AS first_order
                       FROM candidates
                      WHERE action IN ('new', 'matching', 'changed') AND collision_group IS NOT NULL
                      GROUP BY collision_group
                     HAVING COUNT(*) > 1 AND SUM(action = 'changed') > 0) AS groups
               JOIN candidates AS first
                 ON first.section * 9007199254740991 + first.position = groups.first_order
              ORDER BY groups.first_order",
        )?;
        let keys = statement.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(keys)
    }

    pub fn conflicts(&self) -> Result<Vec<(String, Option<String>, String)>> {
        let mut statement =
            self.connection().prepare("SELECT match_key, record_id, message FROM conflicts ORDER BY conflict_order")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn count(&self, action: Action) -> Result<u64> {
        let count: i64 = self.connection().query_row(
            "SELECT COUNT(*) FROM candidates WHERE action = ?1",
            [action.as_str()],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn route_points(&self) -> Result<u64> {
        let count: i64 = self.connection().query_row(
            "SELECT COALESCE(SUM(route_points), 0) FROM candidates WHERE action IN ('new', 'matching', 'changed')",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Planned records in plan order, in bounded pages. `actions` filters them.
    pub fn planned_page(
        &self,
        actions: &[Action],
        after: PlanCursor,
        limit: usize,
    ) -> Result<(Vec<PlannedRecord>, PlanCursor)> {
        let (section, position) = after.unwrap_or((-1, -1));
        let filter = actions.iter().map(|action| format!("'{}'", action.as_str())).collect::<Vec<_>>().join(", ");
        let mut statement = self.connection().prepare_cached(&format!(
            "SELECT section, position, record_id, record_type, action, current_revision, match_key, data_json
               FROM candidates
              WHERE action IN ({filter}) AND (section, position) > (?1, ?2)
              ORDER BY section, position LIMIT ?3"
        ))?;
        let mut last = None;
        let rows = statement
            .query_map(params![section, position, limit as i64], |row| {
                Ok((
                    (row.get::<_, i64>(0)?, row.get::<_, i64>(1)?),
                    PlannedRecord {
                        record_id: row.get(2)?,
                        record_type: row.get(3)?,
                        action: Action::parse(&row.get::<_, String>(4)?).unwrap_or(Action::Conflict),
                        current_revision: row.get::<_, Option<i64>>(5)?.map(|revision| revision as u64),
                        match_key: row.get(6)?,
                        data_json: row.get(7)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(order, record)| {
                last = Some(order);
                record
            })
            .collect();
        Ok((rows, last))
    }

    /// The record each written or repeated candidate names, in plan order: what
    /// a keyed route lists as `record_ids`. A repeat names its earlier record again.
    pub fn record_ids(&self) -> Result<Vec<String>> {
        let mut statement = self.connection().prepare(
            "SELECT record_id FROM candidates
              WHERE action IN ('new', 'matching', 'changed', 'repeat')
              ORDER BY section, position",
        )?;
        let ids = statement.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(ids)
    }

    /// Every planned record with one of `actions`, in plan order.
    pub fn planned(&self, actions: &[Action]) -> Result<Vec<PlannedRecord>> {
        let mut out = Vec::new();
        let mut after = None;
        loop {
            let (page, last) = self.planned_page(actions, after, crate::input_policy::IMPORT_BATCH_RECORDS)?;
            if page.is_empty() {
                return Ok(out);
            }
            out.extend(page);
            after = last;
        }
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = connection.close();
        }
        // Only the directory this value created is removed.
        if let Err(error) = std::fs::remove_dir_all(&self.directory) {
            if error.kind() != std::io::ErrorKind::NotFound {
                let _ = Error::Io(error);
            }
        }
    }
}
