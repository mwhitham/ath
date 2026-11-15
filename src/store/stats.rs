//! `ath stats`: what a database holds.
//!
//! Counts, sources and their date ranges, 90-day device baselines, sample
//! series coverage, benchmarks, and predictions by author. Baselines exclude
//! manual sources and never pool different sources. The plain-text rendering
//! is here because the JavaScript package exports it as `renderDatabaseStats`.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::Serialize;

use super::{database_revision, read_metadata, Store};
use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::grade::js_round;
use crate::model::{point_measurement_unit, time};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceStats {
    pub record_id: String,
    pub source_type: String,
    pub data_writer: String,
    pub data_route: String,
    pub vendor: Option<String>,
    pub sensor_type: Option<String>,
    pub current_record_count: u64,
    pub first_recorded_at: Option<String>,
    pub last_recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BaselineStats {
    pub source_id: String,
    pub quantity: String,
    /// Absent when the stored quantity is not a known point measurement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub mean: f64,
    pub standard_deviation: f64,
    pub observation_count: u64,
    pub from: String,
    pub to: String,
    pub window_days: u32,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesCoverageStats {
    pub source_id: String,
    pub quantity: String,
    pub unit: String,
    pub sample_count: u64,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurrentRecordCounts {
    pub total: u64,
    /// Count per record type, in record-type order.
    pub by_type: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BenchmarkCounts {
    pub defined: u64,
    pub results: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuthorCount {
    pub author: String,
    pub predictions: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionCounts {
    pub total: u64,
    pub graded: u64,
    pub by_author: Vec<AuthorCount>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatabaseStats {
    pub athleticstandard_version: String,
    pub database_id: String,
    pub database_revision: u64,
    /// The athlete profile's stored data, or null.
    pub athlete: Option<Value>,
    pub current_records: CurrentRecordCounts,
    pub sources: Vec<SourceStats>,
    pub baselines: Vec<BaselineStats>,
    pub series: Vec<SeriesCoverageStats>,
    pub benchmarks: BenchmarkCounts,
    pub predictions: PredictionCounts,
}

pub const BASELINE_WINDOW_DAYS: u32 = 90;
pub const BASELINE_METHOD: &str = "sample_mean_and_standard_deviation";

fn rounded(value: f64) -> f64 {
    js_round(value * 10.0) / 10.0
}

fn mean(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |sum, value| sum + value) / values.len() as f64
}

fn standard_deviation(values: &[f64], average: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let squares = values.iter().fold(0.0, |sum, value| sum + (value - average) * (value - average));
    (squares / (values.len() - 1) as f64).sqrt()
}

const SOURCES_SQL: &str = "
    WITH measured AS (
      SELECT record_id, source_id, recorded_at AS at FROM point_measurements
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM measurement_samples
      UNION ALL
      SELECT record_id, source_id, start_at AS at FROM sleep_sessions
      UNION ALL
      SELECT record_id, source_id, start_at AS at FROM workout_sessions
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM benchmark_results
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM provider_summaries
      UNION ALL
      SELECT record_id, source_id, recorded_at AS at FROM provider_scores
    ),
    current_measured AS (
      SELECT measured.source_id, measured.at
      FROM measured
      JOIN records ON records.record_id = measured.record_id
      WHERE records.is_deleted = 0
    )
    SELECT
      sources.record_id,
      sources.source_type,
      sources.data_writer,
      sources.data_route,
      sources.vendor,
      sources.sensor_type,
      COUNT(current_measured.source_id) AS current_record_count,
      MIN(current_measured.at) AS first_recorded_at,
      MAX(current_measured.at) AS last_recorded_at
    FROM sources
    JOIN records
      ON records.record_id = sources.record_id
     AND records.is_deleted = 0
    LEFT JOIN current_measured ON current_measured.source_id = sources.record_id
    GROUP BY sources.record_id
    ORDER BY sources.data_writer, sources.data_route, sources.record_id";

const BASELINE_GROUPS_SQL: &str = "
    SELECT
      point_measurements.source_id,
      point_measurements.quantity,
      MAX(point_measurements.recorded_at) AS latest_at
    FROM point_measurements
    JOIN records ON records.record_id = point_measurements.record_id
    JOIN sources ON sources.record_id = point_measurements.source_id
    WHERE
      records.is_deleted = 0
      AND sources.source_type <> 'manual'
      AND point_measurements.recorded_at IS NOT NULL
    GROUP BY point_measurements.source_id, point_measurements.quantity
    ORDER BY point_measurements.source_id, point_measurements.quantity";

const BASELINE_OBSERVATIONS_SQL: &str = "
    SELECT point_measurements.standard_value, point_measurements.recorded_at
    FROM point_measurements
    JOIN records ON records.record_id = point_measurements.record_id
    WHERE
      records.is_deleted = 0
      AND point_measurements.source_id = ?1
      AND point_measurements.quantity = ?2
      AND point_measurements.recorded_at >= ?3
      AND point_measurements.recorded_at <= ?4
    ORDER BY point_measurements.recorded_at, point_measurements.record_id";

const SERIES_SQL: &str = "
    SELECT
      measurement_samples.source_id,
      measurement_samples.quantity,
      measurement_samples.standard_unit,
      COUNT(*) AS sample_count,
      MIN(measurement_samples.recorded_at) AS first_at,
      MAX(measurement_samples.recorded_at) AS last_at
    FROM measurement_samples
    JOIN records ON records.record_id = measurement_samples.record_id
    WHERE records.is_deleted = 0
    GROUP BY
      measurement_samples.source_id,
      measurement_samples.quantity,
      measurement_samples.standard_unit
    ORDER BY measurement_samples.source_id, measurement_samples.quantity";

const AUTHORS_SQL: &str = "
    SELECT
      COALESCE(
        json_extract(predictions.data_json, '$.authorship.author.agent') ||
          ' running ' ||
          json_extract(predictions.data_json, '$.authorship.author.model'),
        json_extract(predictions.data_json, '$.authorship.author.tool'),
        'athlete'
      ) AS author,
      COUNT(*) AS prediction_count
    FROM predictions
    JOIN records ON records.record_id = predictions.record_id
    WHERE records.is_deleted = 0
    GROUP BY author
    ORDER BY prediction_count DESC, author";

/// Read the statistics in one snapshot.
pub fn database_stats(store: &Store) -> Result<DatabaseStats> {
    let snapshot = rusqlite::Transaction::new_unchecked(store.connection(), rusqlite::TransactionBehavior::Deferred)?;
    let stats = read_stats(&snapshot);
    snapshot.rollback()?;
    stats
}

fn text(value: rusqlite::types::Value) -> Option<String> {
    match value {
        rusqlite::types::Value::Null => None,
        rusqlite::types::Value::Text(text) => Some(text),
        rusqlite::types::Value::Integer(number) => Some(json::format_number(number as f64)),
        rusqlite::types::Value::Real(number) => Some(json::format_number(number)),
        rusqlite::types::Value::Blob(_) => Some(String::new()),
    }
}

fn read_stats(connection: &Connection) -> Result<DatabaseStats> {
    let metadata = read_metadata(connection)?;
    let revision = database_revision(connection)?;

    let athlete = {
        let mut statement = connection.prepare(
            "SELECT athlete_profile.data_json
             FROM athlete_profile
             JOIN records ON records.record_id = athlete_profile.record_id
             WHERE records.is_deleted = 0",
        )?;
        let mut rows = statement.query([])?;
        match rows.next()? {
            Some(row) => Some(json::parse(&row.get::<_, String>(0)?)?),
            None => None,
        }
    };

    let mut by_type = BTreeMap::new();
    {
        let mut statement = connection.prepare(
            "SELECT record_type, COUNT(*) FROM records WHERE is_deleted = 0 GROUP BY record_type ORDER BY record_type",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            by_type.insert(row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64);
        }
    }
    let total = by_type.values().sum();

    let mut sources = Vec::new();
    {
        let mut statement = connection.prepare(SOURCES_SQL)?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            sources.push(SourceStats {
                record_id: row.get(0)?,
                source_type: row.get(1)?,
                data_writer: row.get(2)?,
                data_route: row.get(3)?,
                vendor: row.get(4)?,
                sensor_type: row.get(5)?,
                current_record_count: row.get::<_, i64>(6)? as u64,
                first_recorded_at: text(row.get(7)?),
                last_recorded_at: text(row.get(8)?),
            });
        }
    }

    let mut baselines = Vec::new();
    {
        let mut groups = connection.prepare(BASELINE_GROUPS_SQL)?;
        let mut observations = connection.prepare(BASELINE_OBSERVATIONS_SQL)?;
        let mut rows = groups.query([])?;
        while let Some(group) = rows.next()? {
            let source_id: String = group.get(0)?;
            let quantity: String = group.get(1)?;
            let latest_at: String = group.get(2)?;
            let latest = crate::model::js_date_parse(&latest_at)
                .ok_or_else(|| Error::other(format!("Invalid time value: {latest_at}")))?;
            let cutoff = time::format_utc_millis(latest - i64::from(BASELINE_WINDOW_DAYS) * 86_400_000);
            let mut values = Vec::new();
            let mut first = None;
            let mut last = None;
            let mut observed = observations.query(rusqlite::params![source_id, quantity, cutoff, latest_at])?;
            while let Some(row) = observed.next()? {
                values.push(row.get::<_, f64>(0)?);
                let recorded_at = text(row.get(1)?).unwrap_or_default();
                if first.is_none() {
                    first = Some(recorded_at.clone());
                }
                last = Some(recorded_at);
            }
            if values.is_empty() {
                continue;
            }
            let average = mean(&values);
            baselines.push(BaselineStats {
                unit: point_measurement_unit(&quantity).map(str::to_string),
                source_id,
                quantity,
                mean: rounded(average),
                standard_deviation: rounded(standard_deviation(&values, average)),
                observation_count: values.len() as u64,
                from: first.unwrap_or_default(),
                to: last.unwrap_or_default(),
                window_days: BASELINE_WINDOW_DAYS,
                method: BASELINE_METHOD.to_string(),
            });
        }
    }

    let mut series = Vec::new();
    {
        let mut statement = connection.prepare(SERIES_SQL)?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            series.push(SeriesCoverageStats {
                source_id: row.get(0)?,
                quantity: row.get(1)?,
                unit: row.get(2)?,
                sample_count: row.get::<_, i64>(3)? as u64,
                from: text(row.get(4)?).unwrap_or_default(),
                to: text(row.get(5)?).unwrap_or_default(),
            });
        }
    }

    let mut by_author = Vec::new();
    {
        let mut statement = connection.prepare(AUTHORS_SQL)?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            by_author.push(AuthorCount {
                author: text(row.get(0)?).unwrap_or_else(|| "null".to_string()),
                predictions: row.get::<_, i64>(1)? as u64,
            });
        }
    }

    let count = |name: &str| by_type.get(name).copied().unwrap_or(0);
    Ok(DatabaseStats {
        athleticstandard_version: metadata.athleticstandard_version,
        database_id: metadata.database_id,
        database_revision: revision,
        athlete,
        benchmarks: BenchmarkCounts { defined: count("benchmark_definition"), results: count("benchmark_result") },
        predictions: PredictionCounts { total: count("prediction"), graded: count("prediction_grade"), by_author },
        current_records: CurrentRecordCounts { total, by_type },
        sources,
        baselines,
        series,
    })
}

/// The first `count` UTF-16 units of `text`, as `String.prototype.slice`.
fn js_slice(text: &str, count: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().take(count).collect();
    String::from_utf16_lossy(&units)
}

/// The human `ath stats` overview, as 2.1.0's `renderDatabaseStats`.
pub fn render_database_stats(stats: &DatabaseStats) -> String {
    let athlete_name = stats
        .athlete
        .as_ref()
        .and_then(|athlete| athlete.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("unnamed athlete");
    let mut lines = vec![
        format!("{athlete_name} · Athletic Standard {}", stats.athleticstandard_version),
        format!("database revision: {}", stats.database_revision),
        String::new(),
        format!("current records: {}", stats.current_records.total),
    ];
    for (record_type, count) in &stats.current_records.by_type {
        lines.push(format!("  {record_type}: {count}"));
    }

    lines.push(String::new());
    lines.push(format!("sources: {}", stats.sources.len()));
    for source in &stats.sources {
        let period = match &source.first_recorded_at {
            None => String::new(),
            Some(first) => format!(
                ", {} → {}",
                js_slice(first, 10),
                js_slice(source.last_recorded_at.as_deref().unwrap_or("undefined"), 10)
            ),
        };
        lines.push(format!(
            "  {}: {} via {} ({}) — {} record{}{period}",
            source.record_id,
            source.data_writer,
            source.data_route,
            source.source_type,
            source.current_record_count,
            if source.current_record_count == 1 { "" } else { "s" }
        ));
    }

    if !stats.baselines.is_empty() {
        lines.push(String::new());
        lines.push("90-day device baselines (each source stays separate):".to_string());
        for baseline in &stats.baselines {
            lines.push(format!(
                "  {} {}: {}{} (sd {}, n={}, {} → {})",
                baseline.source_id,
                baseline.quantity,
                json::format_number(baseline.mean),
                baseline.unit.as_deref().unwrap_or("undefined"),
                json::format_number(baseline.standard_deviation),
                baseline.observation_count,
                js_slice(&baseline.from, 10),
                js_slice(&baseline.to, 10)
            ));
        }
    }

    if !stats.series.is_empty() {
        lines.push(String::new());
        lines.push("sample series:".to_string());
        for series in &stats.series {
            lines.push(format!(
                "  {} {}: {} samples, {} → {}",
                series.source_id, series.quantity, series.sample_count, series.from, series.to
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "benchmarks defined: {} · results recorded: {}",
        stats.benchmarks.defined, stats.benchmarks.results
    ));
    lines.push(format!("predictions: {} · grades: {}", stats.predictions.total, stats.predictions.graded));
    for author in &stats.predictions.by_author {
        lines.push(format!("  {}: {}", author.author, author.predictions));
    }
    lines.join("\n")
}
