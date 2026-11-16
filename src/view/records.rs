//! `ath records list` and `ath records show`: stable IDs, one record, its
//! retained revisions, and the links that point into and out of it.

use serde::Serialize;

use crate::error::{Error, Result};
use crate::json::{self, Value};
use crate::model::{record_id, CurrentRecord, RecordData, RecordRevision};
use crate::store::read::{self, ListOptions};
use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkDirection {
    // Declared in the order the 2.1.0 program sorted them.
    Incoming,
    Outgoing,
}

impl LinkDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkDirection::Incoming => "incoming",
            LinkDirection::Outgoing => "outgoing",
        }
    }
}

/// One link between two records. `revision` is present when the link names
/// an exact revision of the other record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordLink {
    pub direction: LinkDirection,
    pub relation: &'static str,
    pub record_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
}

/// Table, owner column, target column, relation, and the revision column when
/// the link names an exact revision.
const LINK_COLUMNS: &[(&str, &str, &str, &str, Option<&str>)] = &[
    ("source_devices", "record_id", "source_id", "source", None),
    ("point_measurements", "record_id", "source_id", "source", None),
    ("measurement_samples", "record_id", "source_id", "source", None),
    ("sleep_sessions", "record_id", "source_id", "source", None),
    ("workout_sessions", "record_id", "source_id", "source", None),
    ("benchmark_results", "record_id", "source_id", "source", None),
    ("benchmark_results", "record_id", "benchmark_definition_id", "benchmark_definition", None),
    ("benchmark_results", "record_id", "workout_session_id", "workout_session", None),
    ("provider_summaries", "record_id", "source_id", "source", None),
    ("provider_scores", "record_id", "source_id", "source", None),
    ("record_attachment_links", "owner_record_id", "attachment_record_id", "attachment", None),
    (
        "point_measurement_calculation_inputs",
        "point_measurement_id",
        "evidence_record_id",
        "calculation_input",
        Some("evidence_revision"),
    ),
    ("predictions", "record_id", "benchmark_definition_id", "benchmark_definition", None),
    ("prediction_result_links", "record_id", "prediction_id", "prediction", Some("prediction_revision")),
    (
        "prediction_result_links",
        "record_id",
        "benchmark_result_id",
        "benchmark_result",
        Some("benchmark_result_revision"),
    ),
    (
        "prediction_grades",
        "record_id",
        "prediction_result_link_id",
        "prediction_result_link",
        Some("prediction_result_link_revision"),
    ),
    (
        "prediction_error_analyses",
        "record_id",
        "prediction_grade_id",
        "prediction_grade",
        Some("prediction_grade_revision"),
    ),
    (
        "prediction_error_causes",
        "prediction_error_analysis_id",
        "evidence_record_id",
        "error_cause_evidence",
        Some("evidence_revision"),
    ),
    ("planned_sessions", "record_id", "training_plan_id", "training_plan", None),
    ("planned_sessions", "record_id", "completed_workout_session_id", "completed_workout_session", None),
    ("goals", "record_id", "target_benchmark_definition_id", "target_benchmark_definition", None),
    ("nutrition_model_interpretations", "record_id", "nutrition_entry_id", "nutrition_entry", None),
    (
        "authored_record_evidence",
        "authored_record_id",
        "evidence_record_id",
        "authored_evidence",
        Some("evidence_revision"),
    ),
];

/// Every incoming and outgoing link of a record, sorted by direction,
/// relation, record ID, and revision.
pub fn read_record_links(store: &Store, record_id: &str) -> Result<Vec<RecordLink>> {
    store.read_snapshot(|| {
        let connection = store.connection();
        let mut links = Vec::new();
        for (table, owner, target, relation, revision_column) in LINK_COLUMNS {
            let revision_select = revision_column.map(|column| format!(", {column}")).unwrap_or_default();
            for (direction, sql) in [
                (
                    LinkDirection::Outgoing,
                    format!(
                        "SELECT {target}{revision_select} FROM {table} WHERE {owner} = ?1 AND {target} IS NOT NULL"
                    ),
                ),
                (LinkDirection::Incoming, format!("SELECT {owner}{revision_select} FROM {table} WHERE {target} = ?1")),
            ] {
                let mut statement = connection.prepare_cached(&sql)?;
                let mut rows = statement.query([record_id])?;
                while let Some(row) = rows.next()? {
                    let linked: String = row.get(0)?;
                    let revision = if revision_column.is_some() {
                        Some(row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64)
                    } else {
                        None
                    };
                    links.push(RecordLink { direction, relation, record_id: linked, revision });
                }
            }
        }
        links.sort_by(|left, right| {
            left.direction
                .cmp(&right.direction)
                .then_with(|| left.relation.cmp(right.relation))
                .then_with(|| left.record_id.cmp(&right.record_id))
                .then_with(|| left.revision.unwrap_or(0).cmp(&right.revision.unwrap_or(0)))
        });
        Ok(links)
    })
}

/// The `ath records list --json` object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordList {
    pub file: String,
    #[serde(serialize_with = "super::serialize_records")]
    pub records: Vec<CurrentRecord>,
}

/// List current records in creation order.
pub fn list_records(store: &Store, options: &ListOptions) -> Result<RecordList> {
    Ok(RecordList {
        file: store.path().display().to_string(),
        records: read::list_records(store.connection(), options)?,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShowOptions {
    /// Show this retained revision.
    pub revision: Option<u64>,
    /// Include every retained revision.
    pub history: bool,
    /// Include incoming and outgoing links.
    pub links: bool,
}

/// The `ath records show --json` object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordShow {
    pub file: String,
    #[serde(serialize_with = "super::serialize_record")]
    pub record: CurrentRecord,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "super::serialize_optional_revision")]
    pub shown_revision: Option<RecordRevision>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "super::serialize_optional_revisions")]
    pub history: Option<Vec<RecordRevision>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<RecordLink>>,
}

/// Check a record ID as `RecordId.parse` did.
pub fn checked_record_id(text: &str) -> Result<()> {
    record_id().validate(&Value::String(text.to_string()))?;
    Ok(())
}

/// Show one record, including a deleted one.
pub fn show_record(store: &Store, id: &str, options: ShowOptions) -> Result<RecordShow> {
    store.read_snapshot(|| {
        checked_record_id(id)?;
        let connection = store.connection();
        let record =
            read::get_record(connection, id)?.ok_or_else(|| Error::not_found(format!("record not found: {id}")))?;
        let revisions =
            if options.revision.is_some() || options.history { Some(read::revisions(connection, id)?) } else { None };
        let shown_revision = match options.revision {
            Some(number) => Some(
                revisions
                    .as_ref()
                    .and_then(|all| all.iter().find(|revision| revision.revision == number))
                    .cloned()
                    .ok_or_else(|| {
                        Error::not_found(format!(
                            "record {id} has no revision {number}; current revision is {}",
                            record.current_revision
                        ))
                    })?,
            ),
            None => None,
        };
        let links = if options.links { Some(read_record_links(store, id)?) } else { None };
        Ok(RecordShow {
            file: store.path().display().to_string(),
            record,
            shown_revision,
            history: if options.history { revisions } else { None },
            links,
        })
    })
}

fn record_label(record: &CurrentRecord) -> String {
    let number = json::format_number;
    match &record.data {
        RecordData::AthleteProfile(data) => data.name.clone().unwrap_or_else(|| "athlete profile".into()),
        RecordData::Source(data) => format!("{} via {}", data.data_writer, data.data_route),
        RecordData::PointMeasurement(data) => {
            format!("{} {}{}", data.quantity, number(data.value.standard_value), data.value.standard_unit)
        }
        RecordData::MeasurementSample(data) => {
            format!("{} {}{}", data.quantity, number(data.value.standard_value), data.value.standard_unit)
        }
        RecordData::ProviderSummary(data) => {
            format!("{} {}{}", data.summary_type, number(data.value.standard_value), data.value.standard_unit)
        }
        RecordData::ProviderScore(data) => {
            format!("{} {} {}", data.metric, number(data.value.standard_value), data.scale)
        }
        RecordData::BenchmarkDefinition(data) => data.benchmark_key.clone(),
        RecordData::AthleteReport(data) => data.report_type.as_str().to_string(),
        RecordData::NutritionEntry(data) => data.athlete_description.clone(),
        RecordData::TrainingPlan(data) => data.title.clone(),
        RecordData::PlannedSession(data) => data.title.clone(),
        RecordData::Goal(data) => data.title.clone(),
        _ => String::new(),
    }
}

/// The human text `ath records list` printed.
pub fn render_record_list(records: &[CurrentRecord]) -> String {
    if records.is_empty() {
        return "no records matched".to_string();
    }
    let mut lines =
        vec!["record ID                             revision  type                         summary".to_string()];
    for record in records {
        lines.push(format!(
            "{}  {}  {}  {}{}",
            record.record_id,
            super::pad_start(&record.current_revision.to_string(), 8),
            super::pad_end(record.record_type().as_str(), 27),
            if record.is_deleted { "[deleted] " } else { "" },
            record_label(record)
        ));
    }
    lines.join("\n")
}

/// The human text `ath records show` printed.
pub fn render_record(show: &RecordShow) -> String {
    let record = &show.record;
    let mut lines = vec![
        record.record_id.clone(),
        format!("type: {}", record.record_type()),
        format!("created: {}", record.created_at),
        format!("current revision: {}", record.current_revision),
        format!("deleted: {}", if record.is_deleted { "yes" } else { "no" }),
    ];
    match &show.shown_revision {
        Some(shown) => {
            lines.push(format!("showing revision: {}", shown.revision));
            lines.push(format!("became known: {}", shown.became_known_at));
            lines.push(format!("changed by: {}", json::encode(&shown.changed_by)));
            lines.push(String::new());
            lines.push(json::to_pretty(&shown.data.to_value()));
        }
        None => {
            lines.push(String::new());
            lines.push(json::to_pretty(&record.data.to_value()));
        }
    }
    if let Some(history) = &show.history {
        lines.push(String::new());
        lines.push("history:".into());
        for revision in history {
            lines.push(format!(
                "  revision {} — {} ({})",
                revision.revision,
                revision.became_known_at,
                revision.became_known_at_basis.as_str()
            ));
        }
    }
    if let Some(links) = &show.links {
        lines.push(String::new());
        lines.push("links:".into());
        if links.is_empty() {
            lines.push("  none".into());
        }
        for link in links {
            lines.push(format!(
                "  {} {}: {}{}",
                link.direction.as_str(),
                link.relation,
                link.record_id,
                link.revision.map(|revision| format!("@{revision}")).unwrap_or_default()
            ));
        }
    }
    lines.join("\n")
}
