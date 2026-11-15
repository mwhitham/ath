//! Bounded queries over current and historical records.
//!
//! Every query has a limit that SQL applies before full payloads are decoded.
//! Device queries leave out manual sources unless the caller asks for them,
//! so a hand-entered value never joins a device baseline.
//!
//! An as-of query keeps two times apart. The event time is when something
//! happened. The knowledge time is when this database learned a revision
//! (`record_revisions.became_known_at`). An as-of read returns, for each
//! record, the newest revision known by the cut-off, so a later correction
//! cannot leak into evidence for an earlier time.

use rusqlite::types::Value as Sql;
use rusqlite::Connection;

use super::read::current_from_row;
use crate::error::{Error, Result};
use crate::json::Value;
use crate::model::{utc_timestamp, CurrentRecord, PointMeasurementType, RecordData, RecordType, SeriesQuantity};

pub const DEFAULT_QUERY_LIMIT: u32 = 10_000;
pub const MAX_QUERY_LIMIT: u32 = 100_000;

/// An optional window on an event time. Bounds must use the stored UTC form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimeRange {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeasurementQuery {
    pub range: TimeRange,
    pub quantity: Option<PointMeasurementType>,
    pub source_id: Option<String>,
    pub include_manual: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeriesQuery {
    pub range: TimeRange,
    pub quantity: SeriesQuantity,
    pub source_id: Option<String>,
    pub include_manual: bool,
    pub limit: Option<u32>,
    pub order: Order,
}

impl SeriesQuery {
    pub fn new(quantity: SeriesQuantity) -> Self {
        SeriesQuery {
            range: TimeRange::default(),
            quantity,
            source_id: None,
            include_manual: false,
            limit: None,
            order: Order::Ascending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Sleep,
    Workout,
}

impl SessionType {
    fn table(self) -> &'static str {
        match self {
            SessionType::Sleep => "sleep_sessions",
            SessionType::Workout => "workout_sessions",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionQuery {
    pub range: TimeRange,
    /// Both session types when absent.
    pub session_type: Option<SessionType>,
    pub source_id: Option<String>,
    pub include_manual: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsOfQuery {
    /// The knowledge cut-off, in the stored UTC form.
    pub as_of: String,
    /// Every type when empty.
    pub record_types: Vec<RecordType>,
    pub limit: Option<u32>,
}

impl AsOfQuery {
    /// The 2.1.0 request shape: one `record_type` or a non-empty
    /// `record_types` list, never both.
    pub fn from_parts(
        as_of: String,
        record_type: Option<RecordType>,
        record_types: Option<Vec<RecordType>>,
        limit: Option<u32>,
    ) -> Result<AsOfQuery> {
        let record_types = match (record_type, record_types) {
            (Some(_), Some(_)) => return Err(Error::other("Use record_type or record_types, not both")),
            (_, Some(types)) if types.is_empty() => {
                return Err(Error::other("record_types must contain at least one record type"))
            }
            (_, Some(types)) => types,
            (Some(record_type), None) => vec![record_type],
            (None, None) => Vec::new(),
        };
        Ok(AsOfQuery { as_of, record_types, limit })
    }
}

/// `UtcTimestamp.parse`: the stored UTC form, or a validation error.
pub fn parse_utc_timestamp(text: &str) -> Result<String> {
    utc_timestamp().validate(&Value::String(text.to_string()))?;
    Ok(text.to_string())
}

/// Check a query limit. The default is 10,000 and the maximum 100,000.
pub fn query_limit(value: Option<u32>) -> Result<u32> {
    let limit = value.unwrap_or(DEFAULT_QUERY_LIMIT);
    if !(1..=MAX_QUERY_LIMIT).contains(&limit) {
        return Err(Error::other("Query limit must be between 1 and 100000"));
    }
    Ok(limit)
}

fn time_range_conditions(
    column: &str,
    range: &TimeRange,
    conditions: &mut Vec<String>,
    args: &mut Vec<Sql>,
) -> Result<()> {
    if let Some(from) = &range.from {
        conditions.push(format!("{column} >= ?"));
        args.push(Sql::Text(parse_utc_timestamp(from)?));
    }
    if let Some(to) = &range.to {
        conditions.push(format!("{column} <= ?"));
        args.push(Sql::Text(parse_utc_timestamp(to)?));
    }
    Ok(())
}

const CURRENT_COLUMNS: &str = "records.record_id, records.record_type, records.current_revision, records.created_at,
       records.is_deleted, record_revisions.record_json, record_revisions.extensions_json";

fn collect(connection: &Connection, sql: &str, args: Vec<Sql>) -> Result<Vec<CurrentRecord>> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(args))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(current_from_row(row)?);
    }
    Ok(out)
}

/// Current point measurements in event order.
pub(crate) fn query_measurements(connection: &Connection, query: &MeasurementQuery) -> Result<Vec<CurrentRecord>> {
    let mut conditions = vec!["records.is_deleted = 0".to_string()];
    let mut args = Vec::new();
    if let Some(quantity) = query.quantity {
        conditions.push("point_measurements.quantity = ?".into());
        args.push(Sql::Text(quantity.as_str().into()));
    }
    if let Some(source) = &query.source_id {
        conditions.push("point_measurements.source_id = ?".into());
        args.push(Sql::Text(source.clone()));
    }
    if !query.include_manual {
        conditions.push("sources.source_type <> 'manual'".into());
    }
    time_range_conditions("point_measurements.recorded_at", &query.range, &mut conditions, &mut args)?;
    args.push(Sql::Integer(i64::from(query_limit(query.limit)?)));
    let sql = format!(
        "SELECT {CURRENT_COLUMNS}
         FROM point_measurements
         JOIN records ON records.record_id = point_measurements.record_id
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         JOIN sources ON sources.record_id = point_measurements.source_id
         WHERE {}
         ORDER BY point_measurements.recorded_at, records.record_id
         LIMIT ?",
        conditions.join(" AND ")
    );
    collect(connection, &sql, args)
}

/// Current measurement samples of one quantity in event order.
pub(crate) fn query_series(connection: &Connection, query: &SeriesQuery) -> Result<Vec<CurrentRecord>> {
    let mut conditions = vec!["records.is_deleted = 0".to_string(), "measurement_samples.quantity = ?".to_string()];
    let mut args = vec![Sql::Text(query.quantity.as_str().into())];
    if let Some(source) = &query.source_id {
        conditions.push("measurement_samples.source_id = ?".into());
        args.push(Sql::Text(source.clone()));
    }
    if !query.include_manual {
        conditions.push("sources.source_type <> 'manual'".into());
    }
    time_range_conditions("measurement_samples.recorded_at", &query.range, &mut conditions, &mut args)?;
    args.push(Sql::Integer(i64::from(query_limit(query.limit)?)));
    let direction = match query.order {
        Order::Ascending => "ASC",
        Order::Descending => "DESC",
    };
    let sql = format!(
        "SELECT {CURRENT_COLUMNS}
         FROM measurement_samples
         JOIN records ON records.record_id = measurement_samples.record_id
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         JOIN sources ON sources.record_id = measurement_samples.source_id
         WHERE {}
         ORDER BY measurement_samples.recorded_at {direction}, records.record_id {direction}
         LIMIT ?",
        conditions.join(" AND ")
    );
    collect(connection, &sql, args)
}

fn session_start(record: &CurrentRecord) -> &str {
    match &record.data {
        RecordData::SleepSession(data) => &data.start_at,
        RecordData::WorkoutSession(data) => &data.start_at,
        _ => "",
    }
}

/// Current sleep and workout sessions in start order.
pub(crate) fn query_sessions(connection: &Connection, query: &SessionQuery) -> Result<Vec<CurrentRecord>> {
    let types: &[SessionType] = match query.session_type {
        Some(SessionType::Sleep) => &[SessionType::Sleep],
        Some(SessionType::Workout) => &[SessionType::Workout],
        None => &[SessionType::Sleep, SessionType::Workout],
    };
    let limit = query_limit(query.limit)?;
    let mut records = Vec::new();
    for session_type in types {
        let table = session_type.table();
        let mut conditions = vec!["records.is_deleted = 0".to_string()];
        let mut args = Vec::new();
        if let Some(source) = &query.source_id {
            conditions.push(format!("{table}.source_id = ?"));
            args.push(Sql::Text(source.clone()));
        }
        if !query.include_manual {
            conditions.push("sources.source_type <> 'manual'".into());
        }
        time_range_conditions(&format!("{table}.start_at"), &query.range, &mut conditions, &mut args)?;
        args.push(Sql::Integer(i64::from(limit)));
        let sql = format!(
            "SELECT {CURRENT_COLUMNS}
             FROM {table}
             JOIN records ON records.record_id = {table}.record_id
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             JOIN sources ON sources.record_id = {table}.source_id
             WHERE {}
             ORDER BY {table}.start_at, records.record_id
             LIMIT ?",
            conditions.join(" AND ")
        );
        records.extend(collect(connection, &sql, args)?);
    }
    records.sort_by(|left, right| {
        session_start(left).cmp(session_start(right)).then_with(|| left.record_id.cmp(&right.record_id))
    });
    records.truncate(limit as usize);
    Ok(records)
}

/// Each record as this database knew it at `as_of`: the newest revision whose
/// knowledge time is not after the cut-off, unless a deletion was known by
/// then. Records are in creation order. The `current_revision` of each result
/// is the revision shown, not the record's present revision.
pub(crate) fn query_records_as_of(connection: &Connection, query: &AsOfQuery) -> Result<Vec<CurrentRecord>> {
    let as_of = parse_utc_timestamp(&query.as_of)?;
    let mut args = vec![Sql::Text(as_of.clone())];
    let type_condition = if query.record_types.is_empty() {
        String::new()
    } else {
        for record_type in &query.record_types {
            args.push(Sql::Text(record_type.as_str().into()));
        }
        format!("WHERE candidate_records.record_type IN ({})", vec!["?"; query.record_types.len()].join(", "))
    };
    args.push(Sql::Text(as_of));
    args.push(Sql::Integer(i64::from(query_limit(query.limit)?)));
    let sql = format!(
        "WITH visible_revisions AS (
           SELECT candidate_records.record_id,
                  (SELECT MAX(candidate_revisions.revision)
                   FROM record_revisions AS candidate_revisions
                   WHERE candidate_revisions.record_id = candidate_records.record_id
                     AND candidate_revisions.became_known_at <= ?) AS revision
           FROM records AS candidate_records
           {type_condition}
         )
         SELECT records.record_id, record_revisions.record_type, record_revisions.revision AS current_revision,
                records.created_at, 0 AS is_deleted, record_revisions.record_json, record_revisions.extensions_json
         FROM visible_revisions
         JOIN record_revisions
           ON record_revisions.record_id = visible_revisions.record_id
          AND record_revisions.revision = visible_revisions.revision
         JOIN records ON records.record_id = record_revisions.record_id
         WHERE visible_revisions.revision IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM record_deletions
             WHERE record_deletions.record_id = record_revisions.record_id
               AND record_deletions.deleted_at <= ?
           )
         ORDER BY records.created_at, records.record_id
         LIMIT ?"
    );
    collect(connection, &sql, args)
}

impl super::Store {
    pub fn query_measurements(&self, query: &MeasurementQuery) -> Result<Vec<CurrentRecord>> {
        query_measurements(self.connection(), query)
    }

    pub fn query_series(&self, query: &SeriesQuery) -> Result<Vec<CurrentRecord>> {
        query_series(self.connection(), query)
    }

    pub fn query_sessions(&self, query: &SessionQuery) -> Result<Vec<CurrentRecord>> {
        query_sessions(self.connection(), query)
    }

    pub fn query_records_as_of(&self, query: &AsOfQuery) -> Result<Vec<CurrentRecord>> {
        query_records_as_of(self.connection(), query)
    }
}
