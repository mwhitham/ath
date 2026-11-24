//! Recording the rows a write changes and replaying them on another copy of the
//! same database (D268).
//!
//! A host that keeps an athlete's database on a remote libSQL server runs every
//! library operation on a local working copy. [`Store::capture`] records, in order,
//! every row the operation inserted, updated, or deleted. It does this with
//! temporary triggers that live only on this connection and only while the
//! operation runs, so the operation itself is unchanged. A change rolled back inside
//! the operation is rolled back from the record too, because the record is a
//! temporary table in the same transaction.
//!
//! [`Store::replay_statements`] turns the record into ordinary SQL statements that
//! any SQLite-compatible server can run in the same order. [`Store::replay`] runs
//! them on a local database, guarded the way a server transaction is guarded, so a
//! working copy can catch up and tests can stand in for a server.
//!
//! Tables in [`NOT_REPLAYED`] are never recorded. Pending file work and the
//! replication log describe one copy only. The access epoch lives on the server: a
//! host raises it there directly and checks it when it commits (D262).
//!
//! The change log is replayed with `INSERT OR IGNORE`. The receiving database's
//! triggers write the same rows as the sending copy's triggers did, because both
//! compute the revision from `database_state` before it is raised. Replaying the
//! rows as well keeps deletions that the sender made, such as a restore starting a
//! new log.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::types::{ToSqlOutput, ValueRef};
use rusqlite::{Connection, ToSql};
use serde_json::Value;

use super::{CreateOptions, Store};
use crate::error::{Error, Result};

/// Tables a capture never records and a replay never sends.
pub const NOT_REPLAYED: &[&str] =
    &["internal_pending_file_operations", "internal_replication_log", "internal_access_state"];

/// Tables whose rows the receiving database's triggers also write.
pub const TRIGGER_WRITTEN: &[&str] = &["internal_change_log"];

/// Tables a working copy never copies from the server.
pub const NOT_COPIED: &[&str] = &["internal_pending_file_operations", "internal_replication_log"];

/// One SQLite value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

impl SqlValue {
    fn from_ref(value: ValueRef<'_>) -> rusqlite::Result<SqlValue> {
        Ok(match value {
            ValueRef::Null => SqlValue::Null,
            ValueRef::Integer(number) => SqlValue::Integer(number),
            ValueRef::Real(number) => SqlValue::Real(number),
            ValueRef::Text(bytes) => SqlValue::Text(std::str::from_utf8(bytes)?.to_string()),
            ValueRef::Blob(_) => {
                return Err(rusqlite::Error::InvalidColumnType(0, "a binary value".into(), rusqlite::types::Type::Blob))
            }
        })
    }

    fn from_json(value: &Value) -> Result<SqlValue> {
        Ok(match value {
            Value::Null => SqlValue::Null,
            Value::String(text) => SqlValue::Text(text.clone()),
            Value::Number(number) => match number.as_i64() {
                Some(integer) if !number.to_string().contains(['.', 'e', 'E']) => SqlValue::Integer(integer),
                _ => SqlValue::Real(
                    number.as_f64().ok_or_else(|| Error::other("A changeset holds an unreadable number"))?,
                ),
            },
            _ => return Err(Error::other("A changeset holds a value that is not a SQLite value")),
        })
    }

    fn to_json(&self) -> Value {
        match self {
            SqlValue::Null => Value::Null,
            SqlValue::Integer(number) => Value::from(*number),
            SqlValue::Real(number) => serde_json::Number::from_f64(*number).map(Value::Number).unwrap_or(Value::Null),
            SqlValue::Text(text) => Value::String(text.clone()),
        }
    }
}

impl ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            SqlValue::Null => ToSqlOutput::Borrowed(ValueRef::Null),
            SqlValue::Integer(number) => ToSqlOutput::Borrowed(ValueRef::Integer(*number)),
            SqlValue::Real(number) => ToSqlOutput::Borrowed(ValueRef::Real(*number)),
            SqlValue::Text(text) => ToSqlOutput::Borrowed(ValueRef::Text(text.as_bytes())),
        })
    }
}

/// What a recorded change did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Insert,
    Update,
    Delete,
}

impl ChangeKind {
    fn name(self) -> &'static str {
        match self {
            ChangeKind::Insert => "insert",
            ChangeKind::Update => "update",
            ChangeKind::Delete => "delete",
        }
    }

    fn parse(name: &str) -> Result<ChangeKind> {
        match name {
            "insert" => Ok(ChangeKind::Insert),
            "update" => Ok(ChangeKind::Update),
            "delete" => Ok(ChangeKind::Delete),
            _ => Err(Error::other(format!("A changeset holds an unknown kind of change: {name}"))),
        }
    }
}

/// One row change. `key` holds the old primary-key values for an update or a
/// deletion. `row` holds every column's new value for an insertion or an update.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub table: String,
    pub kind: ChangeKind,
    pub key: Vec<SqlValue>,
    pub row: Vec<SqlValue>,
}

/// The row changes one or more committed writes made, in the order they made them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changeset(pub Vec<Change>);

impl Changeset {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The changeset as JSON text, as `internal_replication_log` stores it.
    pub fn to_json(&self) -> String {
        Value::Array(
            self.0
                .iter()
                .map(|change| {
                    serde_json::json!([
                        change.table,
                        change.kind.name(),
                        change.key.iter().map(SqlValue::to_json).collect::<Vec<_>>(),
                        change.row.iter().map(SqlValue::to_json).collect::<Vec<_>>(),
                    ])
                })
                .collect(),
        )
        .to_string()
    }

    /// Read JSON text written by [`Changeset::to_json`].
    pub fn from_json(text: &str) -> Result<Changeset> {
        let value: Value =
            serde_json::from_str(text).map_err(|error| Error::other(format!("A changeset is not JSON: {error}")))?;
        let entries = value.as_array().ok_or_else(|| Error::other("A changeset must be a JSON array"))?;
        let mut changes = Vec::with_capacity(entries.len());
        for entry in entries {
            let parts = entry
                .as_array()
                .filter(|parts| parts.len() == 4)
                .ok_or_else(|| Error::other("A change must have four parts"))?;
            let table = parts[0].as_str().ok_or_else(|| Error::other("A change must name its table"))?.to_string();
            let kind = ChangeKind::parse(parts[1].as_str().unwrap_or_default())?;
            let values = |part: &Value| -> Result<Vec<SqlValue>> {
                part.as_array()
                    .ok_or_else(|| Error::other("A change's values must be an array"))?
                    .iter()
                    .map(SqlValue::from_json)
                    .collect()
            };
            changes.push(Change { table, kind, key: values(&parts[2])?, row: values(&parts[3])? });
        }
        Ok(Changeset(changes))
    }
}

/// One SQL statement with positional parameters.
///
/// An insertion or an update must change exactly one row. A deletion may change
/// none, because deleting a parent row earlier can already have removed it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayStatement {
    pub kind: ChangeKind,
    pub table: String,
    pub sql: String,
    pub params: Vec<SqlValue>,
}

impl ReplayStatement {
    /// Whether `changed_rows` is the result this statement must have.
    pub fn accepts(&self, changed_rows: u64) -> bool {
        match self.kind {
            ChangeKind::Delete => changed_rows <= 1,
            ChangeKind::Insert if TRIGGER_WRITTEN.contains(&self.table.as_str()) => changed_rows <= 1,
            ChangeKind::Insert | ChangeKind::Update => changed_rows == 1,
        }
    }
}

/// The value an operation returned, the rows it changed, and the state it started
/// from. A server commits the changeset only while its database revision is
/// `base_revision` and its access epoch is `access_epoch`.
#[derive(Debug, Clone)]
pub struct Captured<T> {
    pub value: T,
    pub changeset: Changeset,
    pub base_revision: u64,
    pub new_revision: u64,
    pub access_epoch: u64,
}

/// A table's columns in declared order and its primary-key columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableShape {
    pub name: String,
    pub columns: Vec<String>,
    pub primary_key: Vec<String>,
}

/// Every row of one table, with columns in declared order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRows {
    pub table: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
}

const CAPTURE_TABLE: &str = "temp.ath_captured_changes";

impl Store {
    /// The access epoch. Revoking a grant raises it (D262).
    pub fn access_epoch(&self) -> Result<u64> {
        self.current_access_epoch()
    }

    /// Run `operation` and record, in order, every row it changed.
    ///
    /// The operation runs unchanged, with the same transactions and checks it has
    /// without recording. If it fails, nothing is returned.
    pub fn capture<T>(&mut self, operation: impl FnOnce(&mut Store) -> Result<T>) -> Result<Captured<T>> {
        if self.read_only {
            return Err(Error::refused("This database was opened read-only"));
        }
        if !self.connection.is_autocommit() {
            return Err(Error::other("A recorded operation must start outside a transaction"));
        }
        let base_revision = self.database_revision()?;
        let access_epoch = self.access_epoch()?;
        let shapes = self.table_shapes()?;
        // Changing `temp_store` deletes every temporary table and trigger. A package
        // restore sets it to MEMORY, so set it first: the restore's change is then no
        // change, and the capture survives it.
        self.connection.pragma_update(None, "temp_store", "MEMORY")?;
        self.connection.execute_batch(&capture_triggers(&shapes))?;
        let outcome = operation(self);
        let recorded = read_capture(&self.connection, &shapes);
        let removed = self.connection.execute_batch(&remove_capture_triggers(&shapes));
        let value = outcome?;
        let changeset = recorded?;
        removed?;
        Ok(Captured { value, changeset, base_revision, new_revision: self.database_revision()?, access_epoch })
    }

    /// The tables of this database, each after every table it references through
    /// a foreign key that is checked immediately.
    pub fn table_shapes(&self) -> Result<Vec<TableShape>> {
        table_shapes(&self.connection)
    }

    /// Turn a changeset into SQL statements, in the order the changes were made.
    pub fn replay_statements(&self, changeset: &Changeset) -> Result<Vec<ReplayStatement>> {
        let shapes: BTreeMap<String, TableShape> =
            self.table_shapes()?.into_iter().map(|shape| (shape.name.clone(), shape)).collect();
        let mut statements = Vec::with_capacity(changeset.0.len());
        for change in &changeset.0 {
            if NOT_REPLAYED.contains(&change.table.as_str()) {
                continue;
            }
            let shape = shapes
                .get(&change.table)
                .ok_or_else(|| Error::other(format!("A changeset names an unknown table: {}", change.table)))?;
            let width = shape.columns.len();
            let key_width = shape.primary_key.len();
            let fits = match change.kind {
                ChangeKind::Insert => change.row.len() == width && change.key.is_empty(),
                ChangeKind::Update => change.row.len() == width && change.key.len() == key_width,
                ChangeKind::Delete => change.row.is_empty() && change.key.len() == key_width,
            };
            if !fits {
                return Err(Error::other(format!("A changeset does not match the columns of {}", change.table)));
            }
            let table = quote(&shape.name);
            let (sql, params) = match change.kind {
                ChangeKind::Insert => {
                    let names = shape.columns.iter().map(|name| quote(name)).collect::<Vec<_>>().join(", ");
                    let marks = (1..=width).map(|n| format!("?{n}")).collect::<Vec<_>>().join(", ");
                    let verb =
                        if TRIGGER_WRITTEN.contains(&shape.name.as_str()) { "INSERT OR IGNORE" } else { "INSERT" };
                    (format!("{verb} INTO {table} ({names}) VALUES ({marks})"), change.row.clone())
                }
                ChangeKind::Update => {
                    let set = shape
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(index, name)| format!("{} = ?{}", quote(name), index + 1))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let condition = key_condition(shape, width + 1);
                    let mut params = change.row.clone();
                    params.extend(change.key.iter().cloned());
                    (format!("UPDATE {table} SET {set} WHERE {condition}"), params)
                }
                ChangeKind::Delete => {
                    (format!("DELETE FROM {table} WHERE {}", key_condition(shape, 1)), change.key.clone())
                }
            };
            statements.push(ReplayStatement { kind: change.kind, table: shape.name.clone(), sql, params });
        }
        Ok(statements)
    }

    /// Apply a changeset recorded on another copy, the way a server applies it: in one
    /// immediate transaction that first checks the database revision and access epoch.
    pub fn replay(&mut self, base_revision: u64, access_epoch: u64, changeset: &Changeset) -> Result<()> {
        let statements = self.replay_statements(changeset)?;
        self.transaction(|tx| {
            let revision = super::database_revision(tx)?;
            let epoch = self::access_epoch(tx)?;
            if revision != base_revision || epoch != access_epoch {
                return Err(Error::stale(format!(
                    "The database moved on: revision {revision} and access epoch {epoch}, expected {base_revision} and {access_epoch}"
                )));
            }
            run_statements(tx, &statements)
        })
    }

    /// Commit a changeset the way a server does, and keep it in
    /// `internal_replication_log` so other working copies can catch up (D268).
    ///
    /// The log row is written before the changes, so an erasure in the changeset
    /// deletes it with the rest of the log. The transaction fails unless the database
    /// ends at `new_revision`.
    pub fn commit_replicated(
        &mut self,
        base_revision: u64,
        new_revision: u64,
        access_epoch: u64,
        changeset: &Changeset,
        committed_at: &str,
    ) -> Result<()> {
        let statements = self.replay_statements(changeset)?;
        let text = changeset.to_json();
        self.transaction(|tx| {
            let revision = super::database_revision(tx)?;
            let epoch = self::access_epoch(tx)?;
            if revision != base_revision || epoch != access_epoch {
                return Err(Error::stale(format!(
                    "The database moved on: revision {revision} and access epoch {epoch}, expected {base_revision} and {access_epoch}"
                )));
            }
            tx.execute(
                "INSERT INTO internal_replication_log (database_revision, base_revision, changeset, committed_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![new_revision as i64, base_revision as i64, text, committed_at],
            )?;
            run_statements(tx, &statements)?;
            let ended = super::database_revision(tx)?;
            if ended != new_revision {
                return Err(Error::other(format!("The changeset ended at revision {ended}, not {new_revision}")));
            }
            Ok(())
        })
    }

    /// The logged changesets that bring a copy at `revision` up to date, in order,
    /// as (base revision, new revision, changeset). `None` means the log has a gap,
    /// so the copy must be rebuilt.
    pub fn replication_log_after(&self, revision: u64) -> Result<Option<Vec<(u64, u64, Changeset)>>> {
        let current = self.database_revision()?;
        let mut statement = self.connection.prepare(
            "SELECT base_revision, database_revision, changeset FROM internal_replication_log
             WHERE database_revision > ?1 ORDER BY database_revision",
        )?;
        let rows: Vec<(i64, i64, String)> = statement
            .query_map([revision as i64], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut at = revision;
        let mut entries = Vec::with_capacity(rows.len());
        for (base, new, text) in rows {
            if base as u64 != at {
                return Ok(None);
            }
            entries.push((base as u64, new as u64, Changeset::from_json(&text)?));
            at = new as u64;
        }
        Ok((at == current).then_some(entries))
    }

    /// Every row of `table`, in primary-key order.
    pub fn table_rows(&self, table: &str) -> Result<TableRows> {
        let shape = self
            .table_shapes()?
            .into_iter()
            .find(|shape| shape.name == table)
            .ok_or_else(|| Error::not_found(format!("No table named {table}")))?;
        read_rows(&self.connection, &shape)
    }

    /// Build a working copy at `path` from every row of a database (D268).
    ///
    /// The copy is created with the current schema, emptied, and filled in one
    /// transaction, parents before children. `database_metadata` comes from the
    /// rows, so the copy has the source's database ID. The change log is written
    /// last, from the rows, so the triggers add no entries of their own.
    pub fn build_working_copy(path: impl AsRef<Path>, tables: &[TableRows]) -> Result<Store> {
        let mut store = Store::create(path, CreateOptions::default())?;
        let shapes = store.table_shapes()?;
        let given: BTreeMap<&str, &TableRows> = tables.iter().map(|rows| (rows.table.as_str(), rows)).collect();
        for name in given.keys() {
            if !shapes.iter().any(|shape| shape.name == *name) {
                return Err(Error::other(format!("A working copy was given rows for an unknown table: {name}")));
            }
        }
        store.transaction(|tx| {
            for shape in shapes.iter().rev() {
                tx.execute(&format!("DELETE FROM {}", quote(&shape.name)), [])?;
            }
            let ordered = shapes
                .iter()
                .filter(|shape| shape.name != "internal_change_log")
                .chain(shapes.iter().filter(|shape| shape.name == "internal_change_log"));
            for shape in ordered {
                if NOT_COPIED.contains(&shape.name.as_str()) {
                    continue;
                }
                if shape.name == "internal_change_log" {
                    tx.execute("DELETE FROM internal_change_log", [])?;
                }
                let Some(rows) = given.get(shape.name.as_str()) else {
                    continue;
                };
                if rows.columns != shape.columns {
                    return Err(Error::other(format!("The columns given for {} do not match", shape.name)));
                }
                let names = shape.columns.iter().map(|name| quote(name)).collect::<Vec<_>>().join(", ");
                let marks = (1..=shape.columns.len()).map(|n| format!("?{n}")).collect::<Vec<_>>().join(", ");
                let mut insert =
                    tx.prepare(&format!("INSERT INTO {} ({names}) VALUES ({marks})", quote(&shape.name)))?;
                for row in &rows.rows {
                    if row.len() != shape.columns.len() {
                        return Err(Error::other(format!("A row for {} has the wrong number of values", shape.name)));
                    }
                    insert.execute(rusqlite::params_from_iter(row.iter()))?;
                }
            }
            Ok(())
        })?;
        store.reload_metadata()?;
        Ok(store)
    }
}

/// The SQL scripts that create an empty database at the current schema, in order,
/// for a server that keeps the database instead of a local file (D258). They are
/// the published migrations with the metadata row between the first and the rest,
/// as a new local file gets them. The two SQLite header settings a local file has are
/// left out: a libSQL server makes them read-only, and `database_metadata` is
/// authoritative (D93).
pub fn schema_scripts(database_id: &str, created_at: &str) -> Vec<String> {
    let literal = |text: &str| format!("'{}'", text.replace('\'', "''"));
    let chain = super::migrate::MIGRATION_CHAIN;
    let mut scripts = vec![chain[0].sql.to_string()];
    scripts.push(format!(
        "INSERT INTO database_metadata (singleton_id, application_marker, sql_schema_version, athleticstandard_version, database_id, created_at)
         VALUES (1, {}, 1, '1.0.0', {}, {});
         INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0);",
        literal(super::SQLITE_APPLICATION_MARKER),
        literal(database_id),
        literal(created_at),
    ));
    scripts.extend(chain[1..].iter().map(|step| step.sql.to_string()));
    scripts
}

fn run_statements(connection: &Connection, statements: &[ReplayStatement]) -> Result<()> {
    for statement in statements {
        let changed = connection.execute(&statement.sql, rusqlite::params_from_iter(statement.params.iter()))? as u64;
        if !statement.accepts(changed) {
            return Err(Error::stale(format!(
                "A replayed change to {} changed {changed} rows: {}",
                statement.table, statement.sql
            )));
        }
    }
    Ok(())
}

fn access_epoch(connection: &Connection) -> Result<u64> {
    let epoch: i64 =
        connection
            .query_row("SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1", [], |row| row.get(0))?;
    Ok(epoch as u64)
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn trigger_name(table: &str, kind: ChangeKind) -> String {
    quote(&format!("ath_capture_{table}_{}", kind.name()))
}

fn captured_tables(shapes: &[TableShape]) -> impl Iterator<Item = &TableShape> {
    shapes.iter().filter(|shape| !NOT_REPLAYED.contains(&shape.name.as_str()))
}

fn capture_triggers(shapes: &[TableShape]) -> String {
    let mut sql = format!(
        "CREATE TEMP TABLE IF NOT EXISTS {CAPTURE_TABLE} (
           position INTEGER PRIMARY KEY,
           table_name TEXT NOT NULL,
           change_kind TEXT NOT NULL,
           old_key TEXT NOT NULL,
           new_row TEXT NOT NULL
         );
         DELETE FROM {CAPTURE_TABLE};\n"
    );
    for shape in captured_tables(shapes) {
        let table = quote(&shape.name);
        let name_literal = shape.name.replace('\'', "''");
        let row = |prefix: &str| {
            format!(
                "json_array({})",
                shape.columns.iter().map(|c| format!("{prefix}.{}", quote(c))).collect::<Vec<_>>().join(", ")
            )
        };
        let key = format!(
            "json_array({})",
            shape.primary_key.iter().map(|c| format!("OLD.{}", quote(c))).collect::<Vec<_>>().join(", ")
        );
        for (kind, old_key, new_row) in [
            (ChangeKind::Insert, "'[]'".to_string(), row("NEW")),
            (ChangeKind::Update, key.clone(), row("NEW")),
            (ChangeKind::Delete, key.clone(), "'[]'".to_string()),
        ] {
            let event = match kind {
                ChangeKind::Insert => "INSERT",
                ChangeKind::Update => "UPDATE",
                ChangeKind::Delete => "DELETE",
            };
            sql.push_str(&format!(
                "CREATE TEMP TRIGGER {} AFTER {event} ON main.{table} BEGIN
                   INSERT INTO {CAPTURE_TABLE} (table_name, change_kind, old_key, new_row)
                   VALUES ('{name_literal}', '{}', {old_key}, {new_row});
                 END;\n",
                trigger_name(&shape.name, kind),
                kind.name(),
            ));
        }
    }
    sql
}

fn remove_capture_triggers(shapes: &[TableShape]) -> String {
    let mut sql = String::new();
    for shape in captured_tables(shapes) {
        for kind in [ChangeKind::Insert, ChangeKind::Update, ChangeKind::Delete] {
            sql.push_str(&format!("DROP TRIGGER IF EXISTS temp.{};\n", trigger_name(&shape.name, kind)));
        }
    }
    sql.push_str(&format!("DELETE FROM {CAPTURE_TABLE};\n"));
    sql
}

fn read_capture(connection: &Connection, shapes: &[TableShape]) -> Result<Changeset> {
    let known: BTreeSet<&str> = shapes.iter().map(|shape| shape.name.as_str()).collect();
    let mut statement = connection
        .prepare(&format!("SELECT table_name, change_kind, old_key, new_row FROM {CAPTURE_TABLE} ORDER BY position"))?;
    let rows: Vec<(String, String, String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut changes = Vec::with_capacity(rows.len());
    for (table, kind, old_key, new_row) in rows {
        if !known.contains(table.as_str()) {
            return Err(Error::other(format!("A capture recorded an unknown table: {table}")));
        }
        let values = |text: &str| -> Result<Vec<SqlValue>> {
            let value: Value = serde_json::from_str(text)
                .map_err(|error| Error::other(format!("A captured row is not JSON: {error}")))?;
            value
                .as_array()
                .ok_or_else(|| Error::other("A captured row is not an array"))?
                .iter()
                .map(SqlValue::from_json)
                .collect()
        };
        changes.push(Change { table, kind: ChangeKind::parse(&kind)?, key: values(&old_key)?, row: values(&new_row)? });
    }
    Ok(Changeset(changes))
}

fn key_condition(shape: &TableShape, first_parameter: usize) -> String {
    shape
        .primary_key
        .iter()
        .enumerate()
        .map(|(index, name)| format!("{} = ?{}", quote(name), first_parameter + index))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn read_rows(connection: &Connection, shape: &TableShape) -> Result<TableRows> {
    let names = shape.columns.iter().map(|name| quote(name)).collect::<Vec<_>>().join(", ");
    let order = shape.primary_key.iter().map(|name| quote(name)).collect::<Vec<_>>().join(", ");
    let mut statement = connection.prepare(&format!("SELECT {names} FROM {} ORDER BY {order}", quote(&shape.name)))?;
    let width = shape.columns.len();
    let rows = statement
        .query_map([], |row| (0..width).map(|column| SqlValue::from_ref(row.get_ref(column)?)).collect())?
        .collect::<rusqlite::Result<Vec<Vec<SqlValue>>>>()?;
    Ok(TableRows { table: shape.name.clone(), columns: shape.columns.clone(), rows })
}

/// User tables in creation order, then sorted so a table comes after every table it
/// references through a foreign key that is checked immediately. A foreign key
/// declared `DEFERRABLE INITIALLY DEFERRED` does not order tables, which breaks the
/// one cycle in the schema: `records.current_revision` refers to `record_revisions`.
fn table_shapes(connection: &Connection) -> Result<Vec<TableShape>> {
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
    )?;
    let tables: Vec<(String, String)> =
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let names: BTreeSet<String> = tables.iter().map(|(name, _)| name.clone()).collect();
    let parents: BTreeMap<String, BTreeSet<String>> = tables
        .iter()
        .map(|(name, sql)| {
            let found = immediate_references(sql)
                .into_iter()
                .filter(|parent| parent != name && names.contains(parent))
                .collect();
            (name.clone(), found)
        })
        .collect();
    let mut ordered: Vec<String> = Vec::new();
    let mut placed: BTreeSet<String> = BTreeSet::new();
    while ordered.len() < tables.len() {
        let next = tables
            .iter()
            .map(|(name, _)| name)
            .find(|name| !placed.contains(*name) && parents[*name].iter().all(|parent| placed.contains(parent)))
            .ok_or_else(|| Error::other("The schema's immediate foreign keys form a cycle"))?
            .clone();
        placed.insert(next.clone());
        ordered.push(next);
    }
    let mut shapes = Vec::new();
    for name in ordered {
        let mut info = connection.prepare(&format!("PRAGMA table_info({})", quote(&name)))?;
        let mut columns: Vec<(String, i64)> = info
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let all: Vec<String> = columns.iter().map(|(column, _)| column.clone()).collect();
        columns.retain(|(_, key)| *key > 0);
        columns.sort_by_key(|(_, key)| *key);
        if columns.is_empty() {
            return Err(Error::other(format!("{name} has no primary key, so its rows cannot be replayed")));
        }
        shapes.push(TableShape {
            name,
            columns: all,
            primary_key: columns.into_iter().map(|(column, _)| column).collect(),
        });
    }
    Ok(shapes)
}

/// The tables `sql` references, leaving out references declared
/// `DEFERRABLE INITIALLY DEFERRED`.
fn immediate_references(sql: &str) -> Vec<String> {
    let upper = sql.to_ascii_uppercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = upper[from..].find("REFERENCES ") {
        let start = from + offset + "REFERENCES ".len();
        let name: String = sql[start..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '"')
            .filter(|c| *c != '"')
            .collect();
        let clause = &upper[start..start + clause_end(&upper[start..])];
        if !clause.contains("DEFERRABLE INITIALLY DEFERRED") {
            found.push(name);
        }
        from = start;
    }
    found
}

/// The end of a REFERENCES clause: the next comma or closing parenthesis outside
/// the referenced column list.
fn clause_end(text: &str) -> usize {
    let mut depth = 0i32;
    for (index, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return index,
            ')' => depth -= 1,
            ',' if depth == 0 => return index,
            _ => {}
        }
    }
    text.len()
}
