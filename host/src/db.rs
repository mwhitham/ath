//! One small SQL client for the host's own data: a local SQLite file, or a database on
//! a server that speaks the libSQL protocol. The host uses it for its directory and,
//! in remote mode, to commit changesets to athletes' databases (D268). Record rules
//! never run here; the library runs them on a working copy.

use std::path::Path;
use std::sync::Arc;

use athleticstandard::store::replica::SqlValue;
use tokio::sync::{Mutex, OwnedMutexGuard};

pub type Row = Vec<SqlValue>;

#[derive(Debug)]
pub struct DbError(pub String);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> DbError {
        DbError(error.to_string())
    }
}

impl From<libsql::Error> for DbError {
    fn from(error: libsql::Error) -> DbError {
        DbError(error.to_string())
    }
}

pub type DbResult<T> = Result<T, DbError>;

#[derive(Clone)]
pub enum Db {
    Local(Arc<Mutex<rusqlite::Connection>>),
    Remote(Arc<libsql::Database>),
}

impl Db {
    /// A local SQLite file, created if absent, with foreign keys on and WAL.
    pub fn open_local(path: &Path) -> DbResult<Db> {
        let connection = rusqlite::Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Db::Local(Arc::new(Mutex::new(connection))))
    }

    /// A database on a libSQL server.
    pub async fn open_remote(url: &str, token: &str) -> DbResult<Db> {
        let database = libsql::Builder::new_remote(url.to_string(), token.to_string()).build().await?;
        Ok(Db::Remote(Arc::new(database)))
    }

    pub async fn execute(&self, sql: &str, params: Vec<SqlValue>) -> DbResult<u64> {
        match self {
            Db::Local(connection) => {
                let connection = connection.lock().await;
                Ok(connection.execute(sql, rusqlite::params_from_iter(params.iter()))? as u64)
            }
            Db::Remote(database) => {
                let connection = database.connect()?;
                Ok(connection.execute(sql, libsql::params_from_iter(params.into_iter().map(to_libsql))).await?)
            }
        }
    }

    pub async fn execute_batch(&self, sql: &str) -> DbResult<()> {
        match self {
            Db::Local(connection) => Ok(connection.lock().await.execute_batch(sql)?),
            Db::Remote(database) => {
                database.connect()?.execute_batch(sql).await?;
                Ok(())
            }
        }
    }

    pub async fn query(&self, sql: &str, params: Vec<SqlValue>) -> DbResult<Vec<Row>> {
        match self {
            Db::Local(connection) => query_local(&*connection.lock().await, sql, &params),
            Db::Remote(database) => query_remote(&database.connect()?, sql, params).await,
        }
    }

    /// An immediate write transaction with foreign keys enforced and deferred to
    /// commit. Dropping it without `commit` rolls it back.
    pub async fn transaction(&self) -> DbResult<Tx> {
        match self {
            Db::Local(connection) => {
                let guard = connection.clone().lock_owned().await;
                guard.execute_batch("BEGIN IMMEDIATE; PRAGMA defer_foreign_keys = ON;")?;
                Ok(Tx::Local { guard: Some(guard) })
            }
            Db::Remote(database) => {
                let connection = database.connect()?;
                connection.execute("PRAGMA foreign_keys = ON", ()).await?;
                let enforced = query_remote(&connection, "PRAGMA foreign_keys", Vec::new()).await?;
                if enforced.first().and_then(|row| row.first()) != Some(&SqlValue::Integer(1)) {
                    return Err(DbError("The server does not enforce foreign keys on this connection (D258)".into()));
                }
                let transaction = connection.transaction_with_behavior(libsql::TransactionBehavior::Immediate).await?;
                transaction.execute("PRAGMA defer_foreign_keys = ON", ()).await?;
                Ok(Tx::Remote { transaction: Some(transaction), _connection: connection })
            }
        }
    }
}

pub enum Tx {
    Local { guard: Option<OwnedMutexGuard<rusqlite::Connection>> },
    Remote { transaction: Option<libsql::Transaction>, _connection: libsql::Connection },
}

impl Tx {
    pub async fn execute(&mut self, sql: &str, params: Vec<SqlValue>) -> DbResult<u64> {
        match self {
            Tx::Local { guard } => {
                let connection = guard.as_ref().expect("open transaction");
                Ok(connection.execute(sql, rusqlite::params_from_iter(params.iter()))? as u64)
            }
            Tx::Remote { transaction, .. } => {
                let transaction = transaction.as_ref().expect("open transaction");
                Ok(transaction.execute(sql, libsql::params_from_iter(params.into_iter().map(to_libsql))).await?)
            }
        }
    }

    pub async fn query(&mut self, sql: &str, params: Vec<SqlValue>) -> DbResult<Vec<Row>> {
        match self {
            Tx::Local { guard } => query_local(guard.as_ref().expect("open transaction"), sql, &params),
            Tx::Remote { transaction, .. } => {
                let transaction = transaction.as_ref().expect("open transaction");
                let mut rows =
                    transaction.query(sql, libsql::params_from_iter(params.into_iter().map(to_libsql))).await?;
                collect_remote(&mut rows).await
            }
        }
    }

    pub async fn commit(mut self) -> DbResult<()> {
        match &mut self {
            Tx::Local { guard } => {
                let connection = guard.take().expect("open transaction");
                connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Tx::Remote { transaction, .. } => {
                transaction.take().expect("open transaction").commit().await?;
                Ok(())
            }
        }
    }
}

impl Drop for Tx {
    fn drop(&mut self) {
        if let Tx::Local { guard: Some(connection) } = self {
            let _ = connection.execute_batch("ROLLBACK");
        }
        // A libSQL transaction dropped without commit is rolled back by the client.
    }
}

/// The first value of the first row as an integer.
pub fn first_integer(rows: &[Row]) -> Option<i64> {
    match rows.first().and_then(|row| row.first()) {
        Some(SqlValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

pub fn text(value: &SqlValue) -> Option<&str> {
    match value {
        SqlValue::Text(text) => Some(text),
        _ => None,
    }
}

pub fn integer(value: &SqlValue) -> Option<i64> {
    match value {
        SqlValue::Integer(value) => Some(*value),
        _ => None,
    }
}

fn query_local(connection: &rusqlite::Connection, sql: &str, params: &[SqlValue]) -> DbResult<Vec<Row>> {
    let mut statement = connection.prepare(sql)?;
    let width = statement.column_count();
    let rows = statement
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            (0..width)
                .map(|column| {
                    Ok(match row.get_ref(column)? {
                        rusqlite::types::ValueRef::Null => SqlValue::Null,
                        rusqlite::types::ValueRef::Integer(value) => SqlValue::Integer(value),
                        rusqlite::types::ValueRef::Real(value) => SqlValue::Real(value),
                        rusqlite::types::ValueRef::Text(bytes) => {
                            SqlValue::Text(String::from_utf8_lossy(bytes).into_owned())
                        }
                        rusqlite::types::ValueRef::Blob(_) => {
                            return Err(rusqlite::Error::InvalidColumnType(
                                column,
                                "binary value".into(),
                                rusqlite::types::Type::Blob,
                            ))
                        }
                    })
                })
                .collect()
        })?
        .collect::<rusqlite::Result<Vec<Row>>>()?;
    Ok(rows)
}

async fn query_remote(connection: &libsql::Connection, sql: &str, params: Vec<SqlValue>) -> DbResult<Vec<Row>> {
    let mut rows = connection.query(sql, libsql::params_from_iter(params.into_iter().map(to_libsql))).await?;
    collect_remote(&mut rows).await
}

async fn collect_remote(rows: &mut libsql::Rows) -> DbResult<Vec<Row>> {
    let width = rows.column_count();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let mut values = Vec::with_capacity(width as usize);
        for column in 0..width {
            values.push(match row.get_value(column)? {
                libsql::Value::Null => SqlValue::Null,
                libsql::Value::Integer(value) => SqlValue::Integer(value),
                libsql::Value::Real(value) => SqlValue::Real(value),
                libsql::Value::Text(value) => SqlValue::Text(value),
                libsql::Value::Blob(_) => return Err(DbError("A host query returned a binary value".into())),
            });
        }
        out.push(values);
    }
    Ok(out)
}

fn to_libsql(value: SqlValue) -> libsql::Value {
    match value {
        SqlValue::Null => libsql::Value::Null,
        SqlValue::Integer(value) => libsql::Value::Integer(value),
        SqlValue::Real(value) => libsql::Value::Real(value),
        SqlValue::Text(value) => libsql::Value::Text(value),
    }
}
