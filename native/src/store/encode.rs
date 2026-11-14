//! The one record encoder.
//!
//! A record's current state lives in three places that must agree: the complete
//! JSON in `record_revisions`, a typed row in the record type's table, and
//! ordered child rows for its collections. This module is the only place that
//! knows how a record maps to those rows. Writes, imports, package restore,
//! repair, and `ath check` all use it.

use rusqlite::types::Value as Sql;
use rusqlite::Transaction;

use crate::error::{Error, Result};
use crate::json;
use crate::model::*;

/// The typed table for a record type.
pub fn table_for(record_type: RecordType) -> &'static str {
    use RecordType::*;
    match record_type {
        AthleteProfile => "athlete_profile",
        Source => "sources",
        SourceDevice => "source_devices",
        PointMeasurement => "point_measurements",
        MeasurementSample => "measurement_samples",
        SleepSession => "sleep_sessions",
        WorkoutSession => "workout_sessions",
        BenchmarkResult => "benchmark_results",
        ProviderSummary => "provider_summaries",
        ProviderScore => "provider_scores",
        AthleteReport => "athlete_reports",
        NutritionEntry => "nutrition_entries",
        Attachment => "attachments",
        BenchmarkDefinition => "benchmark_definitions",
        Prediction => "predictions",
        PredictionResultLink => "prediction_result_links",
        PredictionGrade => "prediction_grades",
        PredictionErrorAnalysis => "prediction_error_analyses",
        TrainingPlan => "training_plans",
        PlannedSession => "planned_sessions",
        CoachingNote => "coaching_notes",
        SavedTrainingReview => "saved_training_reviews",
        Goal => "goals",
        NutritionModelInterpretation => "nutrition_model_interpretations",
    }
}

/// Columns that SQL schema steps before 3 do not have.
const STEP_3_WORKOUT_COLUMNS: &[&str] = &["name", "description", "text_result"];

fn text(value: &str) -> Sql {
    Sql::Text(value.to_string())
}

fn opt_text(value: &Option<String>) -> Sql {
    value.as_ref().map_or(Sql::Null, |text| Sql::Text(text.clone()))
}

fn int(value: u64) -> Sql {
    Sql::Integer(value as i64)
}

fn opt_int(value: Option<u64>) -> Sql {
    value.map_or(Sql::Null, int)
}

fn real(value: f64) -> Sql {
    Sql::Real(value)
}

fn authored(meta: &AuthoredRecordMetadata) -> [(&'static str, Sql); 4] {
    [
        ("authored_at", text(&meta.authored_at)),
        ("author_type", text(meta.author.author_type())),
        ("content_version", int(meta.content_version)),
        ("status", text(meta.status.as_str())),
    ]
}

/// The typed-table columns for a record's data, in published column order.
/// `record_id`, `record_type`, and `data_json` are added by the writer.
pub fn typed_columns(data: &RecordData) -> Vec<(&'static str, Sql)> {
    match data {
        RecordData::AthleteProfile(d) => vec![("default_time_zone", opt_text(&d.default_time_zone))],
        RecordData::Source(d) => vec![
            ("source_type", text(d.source_type.as_str())),
            ("data_writer", text(&d.data_writer)),
            ("data_route", text(&d.data_route)),
            ("vendor", opt_text(&d.vendor)),
            ("sensor_type", opt_text(&d.sensor_type)),
        ],
        RecordData::SourceDevice(d) => vec![
            ("source_id", text(&d.source_id)),
            ("name", opt_text(&d.name)),
            ("manufacturer", opt_text(&d.manufacturer)),
            ("model", opt_text(&d.model)),
            ("hardware", opt_text(&d.hardware)),
            ("serial_number", opt_text(&d.serial_number)),
            ("coverage_start_date", opt_text(&d.coverage_start_date)),
            ("coverage_end_date", opt_text(&d.coverage_end_date)),
            ("accepted_record_count", opt_int(d.accepted_record_count)),
        ],
        RecordData::PointMeasurement(d) => vec![
            ("source_id", text(&d.source_id)),
            ("quantity", text(d.quantity.as_str())),
            ("recorded_at", opt_text(&d.recorded_at)),
            ("standard_value", real(d.value.standard_value)),
            ("standard_unit", text(&d.value.standard_unit)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::MeasurementSample(d) => vec![
            ("source_id", text(&d.source_id)),
            ("quantity", text(d.quantity.as_str())),
            ("recorded_at", text(&d.recorded_at)),
            ("standard_value", real(d.value.standard_value)),
            ("standard_unit", text(&d.value.standard_unit)),
            ("duration_ms", opt_int(d.duration_ms)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::SleepSession(d) => vec![
            ("source_id", text(&d.source_id)),
            ("start_at", text(&d.start_at)),
            ("end_at", text(&d.end_at)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::WorkoutSession(d) => vec![
            ("source_id", text(&d.source_id)),
            ("start_at", text(&d.start_at)),
            ("end_at", text(&d.end_at)),
            ("activity_type", opt_text(&d.activity_type)),
            ("name", opt_text(&d.name)),
            ("description", opt_text(&d.description)),
            ("text_result", opt_text(&d.text_result)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::BenchmarkResult(d) => vec![
            ("source_id", text(&d.source_id)),
            ("benchmark_definition_id", text(&d.benchmark_definition_id)),
            ("recorded_at", text(&d.recorded_at)),
            ("workout_session_id", opt_text(&d.workout_session_id)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::ProviderScore(d) => vec![
            ("source_id", text(&d.source_id)),
            ("metric", text(&d.metric)),
            ("recorded_at", opt_text(&d.recorded_at)),
            ("standard_value", real(d.value.standard_value)),
            ("standard_unit", text(&d.value.standard_unit)),
            ("scale", text(&d.scale)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::ProviderSummary(d) => vec![
            ("source_id", text(&d.source_id)),
            ("summary_type", text(&d.summary_type)),
            ("recorded_at", opt_text(&d.recorded_at)),
            ("standard_value", real(d.value.standard_value)),
            ("standard_unit", text(&d.value.standard_unit)),
            ("provider_record_id", opt_text(&d.provider_record_id)),
            ("received_at", text(&d.received_at)),
        ],
        RecordData::AthleteReport(d) => {
            vec![("report_type", text(d.report_type.as_str())), ("reported_at", text(&d.reported_at))]
        }
        RecordData::NutritionEntry(d) => {
            vec![("reported_at", text(&d.reported_at)), ("athlete_description", text(&d.athlete_description))]
        }
        RecordData::Attachment(d) => vec![
            ("file_name", text(&d.file_name)),
            ("media_type", text(&d.media_type)),
            ("byte_count", int(d.byte_count)),
            ("sha256", text(&d.sha256)),
        ],
        RecordData::BenchmarkDefinition(d) => vec![
            ("benchmark_key", text(&d.benchmark_key)),
            ("benchmark_type", text(d.benchmark_type.as_str())),
            ("score_type", text(d.score_type.as_str())),
            ("definition", text(&d.definition)),
        ],
        RecordData::Prediction(d) => {
            let mut columns = vec![("benchmark_definition_id", text(&d.benchmark_definition_id))];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::PredictionResultLink(d) => vec![
            ("prediction_id", text(&d.prediction.record_id)),
            ("prediction_revision", int(d.prediction.revision)),
            ("benchmark_result_id", text(&d.benchmark_result.record_id)),
            ("benchmark_result_revision", int(d.benchmark_result.revision)),
            ("linked_at", text(&d.linked_at)),
            ("created_by_type", text(d.created_by.author_type())),
            ("content_version", int(d.content_version)),
            ("status", text(d.status.as_str())),
        ],
        RecordData::PredictionGrade(d) => {
            let mut columns = vec![
                ("prediction_result_link_id", text(&d.prediction_result_link.record_id)),
                ("prediction_result_link_revision", int(d.prediction_result_link.revision)),
            ];
            columns.extend(authored(&d.authorship));
            columns.push(("signed_error", real(d.signed_error)));
            columns.push(("absolute_error_percentage", real(d.absolute_error_percentage)));
            columns.push(("result_in_predicted_range", Sql::Integer(i64::from(d.result_in_predicted_range))));
            columns
        }
        RecordData::PredictionErrorAnalysis(d) => {
            let mut columns = vec![
                ("prediction_grade_id", text(&d.prediction_grade.record_id)),
                ("prediction_grade_revision", int(d.prediction_grade.revision)),
            ];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::TrainingPlan(d) => {
            let mut columns = vec![
                ("title", text(&d.title)),
                ("start_date", opt_text(&d.start_date)),
                ("end_date", opt_text(&d.end_date)),
            ];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::PlannedSession(d) => {
            let mut columns = vec![
                ("training_plan_id", opt_text(&d.training_plan_id)),
                ("title", text(&d.title)),
                ("scheduled_date", opt_text(&d.scheduled_date)),
                ("scheduled_start_at", opt_text(&d.scheduled_start_at)),
                ("completion_status", text(d.completion_status.as_str())),
                ("completed_workout_session_id", opt_text(&d.completed_workout_session_id)),
            ];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::CoachingNote(d) => authored(&d.authorship).to_vec(),
        RecordData::SavedTrainingReview(d) => {
            let mut columns = vec![("period_start", text(&d.period_start)), ("period_end", text(&d.period_end))];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::Goal(d) => {
            let mut columns = vec![
                ("title", text(&d.title)),
                ("target_date", opt_text(&d.target_date)),
                ("target_benchmark_definition_id", opt_text(&d.target_benchmark_definition_id)),
            ];
            columns.extend(authored(&d.authorship));
            columns
        }
        RecordData::NutritionModelInterpretation(d) => {
            let mut columns =
                vec![("nutrition_entry_id", text(&d.nutrition_entry_id)), ("confidence", real(d.confidence))];
            columns.extend(authored(&d.authorship));
            columns
        }
    }
}

/// Rows in one child table that belong to a record.
pub struct ChildRows {
    pub table: &'static str,
    /// The column that names the owning record.
    pub owner_column: &'static str,
    /// Column names after the owner column.
    pub columns: &'static [&'static str],
    pub rows: Vec<Vec<Sql>>,
}

fn json_children<T: serde::Serialize>(
    table: &'static str,
    owner_column: &'static str,
    columns: &'static [&'static str],
    items: &[T],
) -> ChildRows {
    ChildRows {
        table,
        owner_column,
        columns,
        rows: items
            .iter()
            .enumerate()
            .map(|(index, item)| vec![Sql::Integer(index as i64), Sql::Text(json::encode(item))])
            .collect(),
    }
}

/// Every child collection a record type owns, in the order they are replaced.
/// A collection the data omits yields an empty set, which removes old rows.
pub fn child_rows(data: &RecordData) -> Vec<ChildRows> {
    let mut children = Vec::new();
    match data {
        RecordData::PointMeasurement(d) => {
            let calculations = d.calculations.as_deref().unwrap_or(&[]);
            children.push(json_children(
                "point_measurement_calculations",
                "point_measurement_id",
                &["calculation_order", "calculation_json"],
                calculations,
            ));
            let mut inputs = Vec::new();
            for (calculation_index, calculation) in calculations.iter().enumerate() {
                for (input_index, input) in calculation.input_records.iter().enumerate() {
                    inputs.push(vec![
                        Sql::Integer(calculation_index as i64),
                        Sql::Integer(input_index as i64),
                        text(&input.record_id),
                        int(input.revision),
                    ]);
                }
            }
            children.push(ChildRows {
                table: "point_measurement_calculation_inputs",
                owner_column: "point_measurement_id",
                columns: &["calculation_order", "input_order", "evidence_record_id", "evidence_revision"],
                rows: inputs,
            });
        }
        RecordData::WorkoutSession(d) => {
            children.push(json_children(
                "workout_session_segments",
                "workout_session_id",
                &["segment_order", "segment_json"],
                d.segments.as_deref().unwrap_or(&[]),
            ));
            children.push(json_children(
                "workout_route_points",
                "workout_session_id",
                &["point_order", "point_json"],
                d.route.as_deref().unwrap_or(&[]),
            ));
        }
        RecordData::BenchmarkResult(d) => children.push(json_children(
            "benchmark_result_segments",
            "benchmark_result_id",
            &["segment_order", "segment_json"],
            d.segments.as_deref().unwrap_or(&[]),
        )),
        RecordData::AthleteReport(_) => children.push(attachment_links(data)),
        RecordData::NutritionEntry(d) => {
            children.push(json_children(
                "reported_nutrition_items",
                "nutrition_entry_id",
                &["item_order", "item_json"],
                &d.reported_items,
            ));
            children.push(attachment_links(data));
        }
        RecordData::PredictionErrorAnalysis(d) => children.push(ChildRows {
            table: "prediction_error_causes",
            owner_column: "prediction_error_analysis_id",
            columns: &["cause_order", "evidence_record_id", "evidence_revision", "explanation"],
            rows: d
                .causes
                .iter()
                .enumerate()
                .map(|(index, cause)| {
                    vec![
                        Sql::Integer(index as i64),
                        text(&cause.evidence.record_id),
                        int(cause.evidence.revision),
                        text(&cause.explanation),
                    ]
                })
                .collect(),
        }),
        RecordData::PlannedSession(d) => children.push(json_children(
            "planned_session_blocks",
            "planned_session_id",
            &["block_order", "block_json"],
            &d.blocks,
        )),
        _ => {}
    }
    children.push(ChildRows {
        table: "provider_reporting_periods",
        owner_column: "owner_record_id",
        columns: &[
            "period_order",
            "period_type",
            "provider_period_id",
            "provider_date",
            "start_at",
            "end_at",
            "period_json",
        ],
        rows: data
            .provider_reporting_periods()
            .iter()
            .enumerate()
            .map(|(index, period)| {
                vec![
                    Sql::Integer(index as i64),
                    text(period.period_type.as_str()),
                    opt_text(&period.provider_period_id),
                    opt_text(&period.provider_date),
                    opt_text(&period.start_at),
                    opt_text(&period.end_at),
                    Sql::Text(json::encode(period)),
                ]
            })
            .collect(),
    });
    children.push(ChildRows {
        table: "authored_record_evidence",
        owner_column: "authored_record_id",
        columns: &["evidence_order", "evidence_record_id", "evidence_revision"],
        rows: data
            .authorship()
            .map(|meta| meta.evidence.as_slice())
            .unwrap_or(&[])
            .iter()
            .enumerate()
            .map(|(index, evidence)| {
                vec![Sql::Integer(index as i64), text(&evidence.record_id), int(evidence.revision)]
            })
            .collect(),
    });
    children
}

fn attachment_links(data: &RecordData) -> ChildRows {
    ChildRows {
        table: "record_attachment_links",
        owner_column: "owner_record_id",
        columns: &["attachment_order", "attachment_record_id"],
        rows: data
            .attachment_ids()
            .iter()
            .enumerate()
            .map(|(index, id)| vec![Sql::Integer(index as i64), text(id)])
            .collect(),
    }
}

/// Which optional tables and columns exist at a SQL schema step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub sql_schema_version: u32,
}

impl Layout {
    pub fn current() -> Self {
        Layout { sql_schema_version: super::migrate::SQL_SCHEMA_VERSION }
    }

    pub fn has_column(self, table: &str, column: &str) -> bool {
        !(self.sql_schema_version < 3 && table == "workout_sessions" && STEP_3_WORKOUT_COLUMNS.contains(&column))
    }

    pub fn has_table(self, table: &str) -> bool {
        !(self.sql_schema_version < 3 && table == "workout_route_points")
    }
}

/// Write a record's current typed row and child rows. `record_json` is the
/// exact text stored in the current revision.
pub fn write_current_state(
    tx: &Transaction<'_>,
    layout: Layout,
    record_id: &str,
    data: &RecordData,
    record_json: &str,
) -> Result<()> {
    let record_type = data.record_type();
    let table = table_for(record_type);
    let fields: Vec<(&'static str, Sql)> =
        typed_columns(data).into_iter().filter(|(column, _)| layout.has_column(table, column)).collect();
    let mut columns = vec!["record_id", "record_type"];
    columns.extend(fields.iter().map(|(column, _)| *column));
    columns.push("data_json");
    let mut values = vec![text(record_id), text(record_type.as_str())];
    values.extend(fields.into_iter().map(|(_, value)| value));
    values.push(text(record_json));
    let updates: Vec<String> = columns
        .iter()
        .filter(|column| **column != "record_id")
        .map(|column| format!("{column} = excluded.{column}"))
        .collect();
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT(record_id) DO UPDATE SET {}",
        columns.join(", "),
        vec!["?"; columns.len()].join(", "),
        updates.join(", ")
    );
    tx.prepare_cached(&sql)?.execute(rusqlite::params_from_iter(values))?;

    for child in child_rows(data) {
        if !layout.has_table(child.table) {
            if child.rows.is_empty() {
                continue;
            }
            return Err(Error::refused(format!(
                "{} is missing. Run ath migrate before writing this record.",
                child.table
            )));
        }
        tx.prepare_cached(&format!("DELETE FROM {} WHERE {} = ?1", child.table, child.owner_column))?
            .execute([record_id])?;
        if child.rows.is_empty() {
            continue;
        }
        let sql = format!(
            "INSERT INTO {} ({}, {}) VALUES ({})",
            child.table,
            child.owner_column,
            child.columns.join(", "),
            vec!["?"; child.columns.len() + 1].join(", ")
        );
        let mut statement = tx.prepare_cached(&sql)?;
        for row in child.rows {
            let mut values = vec![text(record_id)];
            values.extend(row);
            statement.execute(rusqlite::params_from_iter(values))?;
        }
    }
    Ok(())
}

/// Rebuild a record's typed row and children from its current revision text.
/// This repairs storage; it does not create a new revision.
pub fn rebuild_from_current_revision(tx: &Transaction<'_>, layout: Layout, record_id: &str) -> Result<()> {
    let (record_type, record_json): (String, String) = tx
        .query_row(
            "SELECT records.record_type, record_revisions.record_json
         FROM records
         JOIN record_revisions
           ON record_revisions.record_id = records.record_id
          AND record_revisions.revision = records.current_revision
         WHERE records.record_id = ?1",
            [record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::other(format!("record {record_id} is missing its current revision"))
            }
            other => other.into(),
        })?;
    let record_type = RecordType::parse(&record_type)
        .ok_or_else(|| Error::other(format!("record {record_id} has unsupported record type {record_type}")))?;
    let value = json::parse(&record_json)?;
    let data =
        RecordData::decode(record_type, &value).map_err(|error| error.under(&[PathSegment::Key("data".into())]))?;
    write_current_state(tx, layout, record_id, &data, &record_json)
}
