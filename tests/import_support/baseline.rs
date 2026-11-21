//! The import baseline comparison text of `tests/helpers/import-baseline.ts`.
//!
//! The retained files in `tests/fixtures/import-baseline/` were written by that
//! helper. This port produces the same bytes from a native database: it drops
//! `received_at` and `imported_at`, replaces record IDs with stable labels, and
//! lists route points, attachments, and relationship columns.

use std::cmp::Ordering;
use std::collections::HashMap;

use athleticstandard::json::{self, compare_utf16, Map, Value};
use rusqlite::Connection;

use super::replace_uuids;

pub const BASELINE_FROM: &str = "d418a8f653f7074ed9e08208c4eb555c391bcbc7";

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
    ("athlete_reports", "record_id", "workout_session_id", "workout_session", None),
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

pub const MEASURED_RECORD_TYPES: &[&str] = &[
    "point_measurement",
    "measurement_sample",
    "sleep_session",
    "workout_session",
    "benchmark_result",
    "provider_summary",
    "provider_score",
];

pub struct StoredRecord {
    pub record_id: String,
    pub record_type: String,
    pub current_revision: i64,
    pub is_deleted: bool,
    pub data: Value,
    pub extensions: Value,
}

struct Link {
    table: String,
    relation: String,
    owner: String,
    target: String,
    revision: Option<i64>,
}

pub struct Snapshot {
    pub records: Vec<StoredRecord>,
    keys: HashMap<String, Vec<(String, String)>>,
    route_points: Vec<(String, i64, Value)>,
    attachments: Vec<(String, String, String, i64, String)>,
    attachment_links: Vec<(String, i64, String)>,
    links: Vec<Link>,
}

fn parse_json(value: Option<String>) -> Value {
    value.map_or(Value::Null, |text| json::parse(&text).unwrap())
}

pub fn read_snapshot(connection: &Connection) -> Snapshot {
    let mut statement = connection
        .prepare(
            "SELECT records.record_id, records.record_type, records.current_revision,
                    records.is_deleted, record_revisions.record_json, record_revisions.extensions_json
               FROM records
               JOIN record_revisions
                 ON record_revisions.record_id = records.record_id
                AND record_revisions.revision = records.current_revision",
        )
        .unwrap();
    let records = statement
        .query_map([], |row| {
            Ok(StoredRecord {
                record_id: row.get(0)?,
                record_type: row.get(1)?,
                current_revision: row.get(2)?,
                is_deleted: row.get::<_, i64>(3)? == 1,
                data: parse_json(row.get(4)?),
                extensions: parse_json(row.get(5)?),
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut keys: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut statement =
        connection.prepare("SELECT record_id, match_key_method, match_key FROM record_import_keys").unwrap();
    for row in statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
        .unwrap()
    {
        let (id, method, key) = row.unwrap();
        keys.entry(id).or_default().push((method, key));
    }
    let mut statement =
        connection.prepare("SELECT workout_session_id, point_order, point_json FROM workout_route_points").unwrap();
    let route_points = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, parse_json(row.get(2)?))))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut statement =
        connection.prepare("SELECT record_id, file_name, media_type, byte_count, sha256 FROM attachments").unwrap();
    let attachments = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut statement = connection
        .prepare("SELECT owner_record_id, attachment_order, attachment_record_id FROM record_attachment_links")
        .unwrap();
    let attachment_links = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut links = Vec::new();
    for (table, owner, target, relation, revision) in LINK_COLUMNS {
        let revision_column = revision.map(|column| format!(", {column} AS revision")).unwrap_or_default();
        let sql = format!(
            "SELECT {owner} AS owner, {target} AS target{revision_column} FROM {table} WHERE {target} IS NOT NULL"
        );
        let mut statement = connection.prepare(&sql).unwrap();
        for row in statement
            .query_map([], |row| {
                Ok(Link {
                    table: table.to_string(),
                    relation: relation.to_string(),
                    owner: row.get(0)?,
                    target: row.get(1)?,
                    revision: if revision.is_some() { Some(row.get(2)?) } else { None },
                })
            })
            .unwrap()
        {
            links.push(row.unwrap());
        }
    }
    Snapshot { records, keys, route_points, attachments, attachment_links, links }
}

fn without_times(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(without_times).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| key.as_str() != "received_at" && key.as_str() != "imported_at")
                .map(|(key, child)| (key.clone(), without_times(child)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn replace_ids(value: &Value, replace: &dyn Fn(&str) -> String) -> Value {
    match value {
        Value::String(text) => Value::String(replace_uuids(text, replace)),
        Value::Array(items) => Value::Array(items.iter().map(|item| replace_ids(item, replace)).collect()),
        Value::Object(map) => {
            Value::Object(map.iter().map(|(key, child)| (key.clone(), replace_ids(child, replace))).collect())
        }
        other => other.clone(),
    }
}

fn without_top_key(data: &Value, key: Option<&str>) -> Value {
    match (key, data) {
        (Some(key), Value::Object(map)) => Value::Object(
            map.iter().filter(|(name, _)| name.as_str() != key).map(|(k, v)| (k.clone(), v.clone())).collect(),
        ),
        _ => data.clone(),
    }
}

fn order(left: &[String], right: &[String]) -> Ordering {
    for index in 0..left.len().max(right.len()) {
        let diff =
            compare_utf16(left.get(index).map_or("", String::as_str), right.get(index).map_or("", String::as_str));
        if diff != Ordering::Equal {
            return diff;
        }
    }
    Ordering::Equal
}

/// `labelRecords`: a label for each record that does not depend on random IDs.
pub fn label_records(snapshot: &Snapshot, without_key: Option<&str>) -> HashMap<String, String> {
    let by_id: HashMap<&str, &StoredRecord> =
        snapshot.records.iter().map(|record| (record.record_id.as_str(), record)).collect();
    let own_identity = |id: &str| -> String {
        let Some(record) = by_id.get(id) else {
            return id.to_string();
        };
        let mut keys: Vec<String> = snapshot
            .keys
            .get(id)
            .map(|keys| {
                keys.iter()
                    .map(|(method, key)| format!("{method}\u{1}{}", replace_uuids(key, |_| "?".into())))
                    .collect()
            })
            .unwrap_or_default();
        keys.sort_by(|left, right| compare_utf16(left, right));
        format!("{}\u{2}{}", record.record_type, keys.join("\u{3}"))
    };
    let substitute = |text: &str| -> String {
        replace_uuids(
            text,
            |id| if by_id.contains_key(id) { format!("<{}>", own_identity(id)) } else { id.to_string() },
        )
    };
    let mut keyed: Vec<(String, Vec<String>)> = Vec::new();
    let mut keyless: Vec<(String, Vec<String>)> = Vec::new();
    for record in &snapshot.records {
        let mut keys: Vec<String> = snapshot
            .keys
            .get(&record.record_id)
            .map(|keys| keys.iter().map(|(method, key)| format!("{method}\u{0}{}", substitute(key))).collect())
            .unwrap_or_default();
        keys.sort_by(|left, right| compare_utf16(left, right));
        if !keys.is_empty() {
            keys.push(record.record_type.clone());
            keyed.push((record.record_id.clone(), keys));
        } else {
            let blanked = replace_ids(&without_times(&without_top_key(&record.data, without_key)), &|id| {
                if by_id.contains_key(id) {
                    "?".into()
                } else {
                    id.to_string()
                }
            });
            keyless
                .push((record.record_id.clone(), vec![record.record_type.clone(), json::to_sorted_string(&blanked)]));
        }
    }
    keyed.sort_by(|left, right| order(&left.1, &right.1));
    keyless.sort_by(|left, right| order(&left.1, &right.1));
    let all: Vec<(String, Vec<String>)> = keyed.into_iter().chain(keyless).collect();
    for pair in all.windows(2) {
        assert_ne!(order(&pair[0].1, &pair[1].1), Ordering::Equal, "two records sort the same way");
    }
    all.into_iter().enumerate().map(|(index, (id, _))| (id, format!("record:{}", index + 1))).collect()
}

fn label_number(label: &str) -> u64 {
    label.trim_start_matches("record:").parse().unwrap_or(0)
}

/// `comparisonString(snapshot, { withoutKey })`.
pub fn comparison_string(snapshot: &Snapshot, without_key: Option<&str>) -> String {
    let labels = label_records(snapshot, without_key);
    let label = |id: &str| labels.get(id).cloned().unwrap_or_else(|| id.to_string());
    let relabel = |value: &Value| replace_ids(value, &|id| label(id));
    let mut records: Vec<Value> = snapshot
        .records
        .iter()
        .map(|record| {
            let mut keys: Vec<(String, String)> = snapshot
                .keys
                .get(&record.record_id)
                .map(|keys| keys.iter().map(|(method, key)| (method.clone(), replace_uuids(key, &label))).collect())
                .unwrap_or_default();
            keys.sort_by(|left, right| compare_utf16(&left.0, &right.0).then_with(|| compare_utf16(&left.1, &right.1)));
            let mut out = Map::new();
            out.insert("label".into(), Value::String(label(&record.record_id)));
            out.insert("record_type".into(), Value::String(record.record_type.clone()));
            out.insert("current_revision".into(), Value::from(record.current_revision));
            out.insert("is_deleted".into(), Value::Bool(record.is_deleted));
            out.insert(
                "import_keys".into(),
                Value::Array(
                    keys.into_iter().map(|(method, key)| serde_json::json!({ "method": method, "key": key })).collect(),
                ),
            );
            out.insert("data".into(), relabel(&without_top_key(&without_times(&record.data), without_key)));
            if !record.extensions.is_null() {
                out.insert("extensions".into(), relabel(&without_times(&record.extensions)));
            }
            Value::Object(out)
        })
        .collect();
    records.sort_by_key(|record| label_number(record["label"].as_str().unwrap()));
    let mut route_points: Vec<Value> = snapshot
        .route_points
        .iter()
        .map(|(workout, order, point)| {
            serde_json::json!({ "workout": label(workout), "order": order, "point": relabel(&without_times(point)) })
        })
        .collect();
    route_points.sort_by(|left, right| {
        label_number(left["workout"].as_str().unwrap())
            .cmp(&label_number(right["workout"].as_str().unwrap()))
            .then_with(|| left["order"].as_i64().cmp(&right["order"].as_i64()))
    });
    let mut attachments: Vec<Value> = snapshot
        .attachments
        .iter()
        .map(|(id, file_name, media_type, byte_count, sha256)| {
            serde_json::json!({
                "record_id": label(id), "file_name": file_name, "media_type": media_type,
                "byte_count": byte_count, "sha256": sha256,
            })
        })
        .collect();
    attachments
        .sort_by(|left, right| compare_utf16(left["sha256"].as_str().unwrap(), right["sha256"].as_str().unwrap()));
    let mut attachment_links: Vec<Value> = snapshot
        .attachment_links
        .iter()
        .map(|(owner, order, attachment)| {
            serde_json::json!({ "owner": label(owner), "order": order, "attachment": label(attachment) })
        })
        .collect();
    attachment_links.sort_by(|left, right| {
        compare_utf16(left["owner"].as_str().unwrap(), right["owner"].as_str().unwrap())
            .then_with(|| left["order"].as_i64().cmp(&right["order"].as_i64()))
    });
    let mut links: Vec<(Link, Value)> = snapshot
        .links
        .iter()
        .map(|link| {
            let mut out = Map::new();
            out.insert("table".into(), Value::String(link.table.clone()));
            out.insert("relation".into(), Value::String(link.relation.clone()));
            out.insert("owner".into(), Value::String(label(&link.owner)));
            out.insert("target".into(), Value::String(label(&link.target)));
            if let Some(revision) = link.revision {
                out.insert("revision".into(), Value::from(revision));
            }
            (
                Link {
                    table: link.table.clone(),
                    relation: link.relation.clone(),
                    owner: label(&link.owner),
                    target: label(&link.target),
                    revision: link.revision,
                },
                Value::Object(out),
            )
        })
        .collect();
    links.sort_by(|(left, _), (right, _)| {
        compare_utf16(&left.table, &right.table)
            .then_with(|| compare_utf16(&left.relation, &right.relation))
            .then_with(|| compare_utf16(&left.owner, &right.owner))
            .then_with(|| compare_utf16(&left.target, &right.target))
            .then_with(|| left.revision.unwrap_or(0).cmp(&right.revision.unwrap_or(0)))
    });
    json::to_deterministic_pretty(&serde_json::json!({
        "baseline_from": BASELINE_FROM,
        "records": records,
        "route_points": route_points,
        "attachments": attachments,
        "attachment_links": attachment_links,
        "links": links.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    }))
}

/// `fieldBasisString(snapshot)`: each record's `field_basis` under the labels
/// built without that key.
pub fn field_basis_string(snapshot: &Snapshot) -> String {
    let labels = label_records(snapshot, Some("field_basis"));
    let mut records: Vec<Value> = snapshot
        .records
        .iter()
        .filter(|record| record.data.get("field_basis").is_some())
        .map(|record| {
            serde_json::json!({
                "label": labels[&record.record_id],
                "record_type": record.record_type,
                "field_basis": record.data["field_basis"],
            })
        })
        .collect();
    records.sort_by_key(|record| label_number(record["label"].as_str().unwrap()));
    json::to_deterministic_pretty(&serde_json::json!({ "decision": "D187", "records": records }))
}
