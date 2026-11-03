//! Write checked records into the SQLite file.
//!
//! The command-line tool still checks field names. This module does the
//! database write: revisions, typed rows, import keys, and deletion records.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{Map, Value};

pub fn apply(conn: &mut Connection, body: &Value) -> Result<Value, String> {
    let actions = body
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("persist needs actions")?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|error| error.to_string())?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    tx.execute_batch("PRAGMA foreign_keys = ON; PRAGMA defer_foreign_keys = ON;")
        .map_err(|error| error.to_string())?;
    let mut touched: Vec<Value> = Vec::new();
    for action in actions {
        let kind = action.get("kind").and_then(Value::as_str).unwrap_or("");
        match kind {
            "expect_database_revision" => expect_database_revision(&tx, action)?,
            "expect_preview" => expect_preview(&tx, action)?,
            "insert" => {
                insert_record(&tx, action)?;
                touched.push(action.clone());
            }
            "revise" => {
                revise_record(&tx, action)?;
                touched.push(action.clone());
            }
            "delete" => delete_record(&tx, action)?,
            "import_key" => import_key(&tx, action)?,
            "validate" => {
                for record in &touched {
                    validate(&tx, record)?;
                }
            }
            "bump" => {}
            "rebuild" => rebuild(&tx, action)?,
            other => return Err(format!("unknown persist action {other}")),
        }
    }
    let bump = actions.iter().any(|action| action.get("kind").and_then(Value::as_str) == Some("bump"));
    let revision = if bump {
        tx.execute(
            "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
            [],
        )
        .map_err(|error| error.to_string())?;
        read_revision(&tx)?
    } else {
        read_revision(&tx)?
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({ "database_revision": revision }))
}

fn read_revision(tx: &Transaction<'_>) -> Result<i64, String> {
    tx.query_row(
        "SELECT current_revision FROM database_state WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

fn expect_database_revision(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let expected = action.get("revision").and_then(Value::as_i64).unwrap_or(-1);
    let current = read_revision(tx)?;
    if current != expected {
        return Err("Database changed after provider import preview; preview it again".into());
    }
    Ok(())
}

fn expect_preview(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let revision = action.get("revision").and_then(Value::as_i64).unwrap_or(-1);
    let row: Option<(i64, i64)> = tx
        .query_row(
            "SELECT current_revision, is_deleted FROM records WHERE record_id = ?1",
            [&record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((current, deleted)) = row else {
        return Err(format!("Expected record does not exist: {record_id}"));
    };
    if current != revision || deleted != 0 {
        return Err(format!("Record {record_id} changed after preview; preview it again"));
    }
    Ok(())
}

fn insert_record(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let record_type = required_str(action, "record_type")?;
    let created_at = required_str(action, "created_at")?;
    let data_json = required_str(action, "data_json")?;
    let changed_by_json = required_str(action, "changed_by_json")?;
    let became_known_at = required_str(action, "became_known_at")?;
    let basis = required_str(action, "became_known_at_basis")?;
    let extensions = optional_text(action, "extensions_json");
    tx.execute(
        "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, ?2, 1, ?3, 0)",
        params![record_id, record_type, created_at],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
         ) VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![record_id, record_type, data_json, extensions, became_known_at, basis, changed_by_json],
    )
    .map_err(|error| error.to_string())?;
    upsert_typed(tx, &record_id, &record_type, &data_json)?;
    Ok(())
}

fn revise_record(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let expected = action.get("expected_revision").and_then(Value::as_i64).unwrap_or(-1);
    let data_json = required_str(action, "data_json")?;
    let changed_by_json = required_str(action, "changed_by_json")?;
    let became_known_at = required_str(action, "became_known_at")?;
    let basis = required_str(action, "became_known_at_basis")?;
    let row: Option<(String, i64, String, i64, String, Option<String>)> = tx
        .query_row(
            "SELECT records.record_type, records.current_revision, records.created_at, records.is_deleted,
                    record_revisions.became_known_at, record_revisions.extensions_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&record_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((record_type, current, _created, deleted, known_at, existing_extensions)) = row else {
        return Err(format!("Record not found: {record_id}"));
    };
    let style = action.get("style").and_then(Value::as_str).unwrap_or("correct");
    if style == "change" {
        if current != expected || deleted != 0 {
            return Err(format!("Record {record_id} changed after preview; preview it again"));
        }
    } else {
        if current != expected {
            return Err(format!(
                "Record {record_id} is at revision {current}, not expected revision {expected}"
            ));
        }
        if deleted != 0 {
            return Err(format!("Deleted record cannot be corrected: {record_id}"));
        }
    }
    if parse_millis(&became_known_at) < parse_millis(&known_at) {
        return Err(format!("Revision knowledge time cannot precede revision {current}"));
    }
    let next = current + 1;
    let extensions = if action.get("extensions_json").is_some() && !action.get("extensions_json").unwrap().is_null() {
        optional_text(action, "extensions_json")
    } else if action.get("keep_extensions").and_then(Value::as_bool) == Some(true) {
        existing_extensions
    } else {
        optional_text(action, "extensions_json")
    };
    tx.execute(
        "INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![record_id, next, record_type, data_json, extensions, became_known_at, basis, changed_by_json, current],
    )
    .map_err(|error| error.to_string())?;
    upsert_typed(tx, &record_id, &record_type, &data_json)?;
    tx.execute(
        "UPDATE records SET current_revision = ?1 WHERE record_id = ?2",
        params![next, record_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn delete_record(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let expected = action.get("expected_revision").and_then(Value::as_i64).unwrap_or(-1);
    let deleted_at = required_str(action, "deleted_at")?;
    let deleted_by_json = required_str(action, "deleted_by_json")?;
    let row: Option<(i64, i64, String)> = tx
        .query_row(
            "SELECT records.current_revision, records.is_deleted, record_revisions.became_known_at
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&record_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((current, deleted, known_at)) = row else {
        return Err(format!("Record not found: {record_id}"));
    };
    if current != expected {
        return Err(format!(
            "Record {record_id} is at revision {current}, not expected revision {expected}"
        ));
    }
    if deleted != 0 {
        return Err(format!("Record is already deleted: {record_id}"));
    }
    if parse_millis(&deleted_at) < parse_millis(&known_at) {
        return Err("Deletion time cannot precede the current revision knowledge time".into());
    }
    tx.execute(
        "INSERT INTO record_deletions (record_id, record_revision, deleted_at, deleted_by_json) VALUES (?1, ?2, ?3, ?4)",
        params![record_id, current, deleted_at, deleted_by_json],
    )
    .map_err(|error| error.to_string())?;
    tx.execute("UPDATE records SET is_deleted = 1 WHERE record_id = ?1", [&record_id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn import_key(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let method = required_str(action, "method")?;
    let key = required_str(action, "key")?;
    let imported_at = required_str(action, "imported_at")?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT record_id FROM record_import_keys WHERE match_key_method = ?1 AND match_key = ?2",
            params![method, key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(existing) = existing {
        if existing != record_id {
            return Err("Provider import key changed after preview".into());
        }
        return Ok(());
    }
    tx.execute(
        "INSERT INTO record_import_keys (record_id, match_key_method, match_key, imported_at) VALUES (?1, ?2, ?3, ?4)",
        params![record_id, method, key, imported_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn rebuild(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_id = required_str(action, "record_id")?;
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT records.record_type, record_revisions.record_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((record_type, data_json)) = row else {
        return Err(format!("record {record_id} is missing its current revision"));
    };
    upsert_typed(tx, &record_id, &record_type, &data_json)?;
    let table = typed_table(&record_type)?;
    tx.execute(
        &format!("UPDATE {table} SET data_json = ?1 WHERE record_id = ?2"),
        params![data_json, record_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn upsert_typed(tx: &Transaction<'_>, record_id: &str, record_type: &str, data_json: &str) -> Result<(), String> {
    let data: Value = serde_json::from_str(data_json).map_err(|error| error.to_string())?;
    let map = data.as_object().ok_or("record data is not an object")?;
    let table = typed_table(record_type)?;
    let usable = typed_fields(record_type, map)?;
    let mut columns = vec!["record_id".to_string(), "record_type".to_string()];
    let mut binds = vec![Bind::Text(record_id.into()), Bind::Text(record_type.into())];
    for (name, value) in usable {
        columns.push(name);
        binds.push(value);
    }
    columns.push("data_json".into());
    binds.push(Bind::Text(data_json.into()));
    let updates = columns
        .iter()
        .filter(|column| column.as_str() != "record_id")
        .map(|column| format!("{column} = excluded.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders}) ON CONFLICT(record_id) DO UPDATE SET {updates}",
        columns.join(", ")
    );
    let params = binds_to_params(&binds);
    tx.execute(&sql, rusqlite::params_from_iter(params.iter()))
        .map_err(|error| error.to_string())?;
    replace_children(tx, record_id, record_type, map, data_json)?;
    Ok(())
}

fn replace_children(
    tx: &Transaction<'_>,
    record_id: &str,
    record_type: &str,
    map: &Map<String, Value>,
    data_json: &str,
) -> Result<(), String> {
    match record_type {
        "point_measurement" => {
            let calculations = map.get("calculations").and_then(Value::as_array);
            replace_json_children(
                tx,
                "point_measurement_calculations",
                "point_measurement_id",
                "calculation_order",
                "calculation_json",
                record_id,
                calculations.map(Vec::as_slice).unwrap_or(&[]),
            )?;
            tx.execute(
                    "DELETE FROM point_measurement_calculation_inputs WHERE point_measurement_id = ?1",
                    [record_id],
                )
                .map_err(|error| error.to_string())?;
                if let Some(calculations) = calculations {
                    for (calculation_index, calculation) in calculations.iter().enumerate() {
                        let inputs = calculation
                            .get("input_records")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        for (input_index, input) in inputs.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO point_measurement_calculation_inputs (
                                    point_measurement_id, calculation_order, input_order, evidence_record_id, evidence_revision
                                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                                params![
                                    record_id,
                                    calculation_index as i64,
                                    input_index as i64,
                                    input.get("record_id").and_then(Value::as_str).unwrap_or(""),
                                    input.get("revision").and_then(Value::as_i64).unwrap_or(0)
                                ],
                            )
                            .map_err(|error| error.to_string())?;
                        }
                    }
                }
        }
        "workout_session" => {
            replace_json_children(
                tx,
                "workout_session_segments",
                "workout_session_id",
                "segment_order",
                "segment_json",
                record_id,
                array(map, "segments"),
            )?;
            replace_json_children(
                tx,
                "workout_route_points",
                "workout_session_id",
                "point_order",
                "point_json",
                record_id,
                array(map, "route"),
            )?;
        }
        "benchmark_result" => {
            replace_json_children(
                tx,
                "benchmark_result_segments",
                "benchmark_result_id",
                "segment_order",
                "segment_json",
                record_id,
                array(map, "segments"),
            )?;
        }
        "athlete_report" => {
            replace_links(tx, record_id, map.get("attachment_ids").and_then(Value::as_array))?;
        }
        "nutrition_entry" => {
            let items = map.get("reported_items").and_then(Value::as_array).ok_or("reported_items")?;
            replace_json_children(
                tx,
                "reported_nutrition_items",
                "nutrition_entry_id",
                "item_order",
                "item_json",
                record_id,
                items,
            )?;
            replace_links(tx, record_id, map.get("attachment_ids").and_then(Value::as_array))?;
        }
        "planned_session" => {
            let blocks = map.get("blocks").and_then(Value::as_array).ok_or("blocks")?;
            replace_json_children(
                tx,
                "planned_session_blocks",
                "planned_session_id",
                "block_order",
                "block_json",
                record_id,
                blocks,
            )?;
        }
        "prediction_error_analysis" => {
            tx.execute(
                "DELETE FROM prediction_error_causes WHERE prediction_error_analysis_id = ?1",
                [record_id],
            )
            .map_err(|error| error.to_string())?;
            let causes = map.get("causes").and_then(Value::as_array).ok_or("causes")?;
            for (index, cause) in causes.iter().enumerate() {
                let evidence = cause.get("evidence").ok_or("cause evidence")?;
                tx.execute(
                    "INSERT INTO prediction_error_causes (
                        prediction_error_analysis_id, cause_order, evidence_record_id, evidence_revision, explanation
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        record_id,
                        index as i64,
                        evidence.get("record_id").and_then(Value::as_str).unwrap_or(""),
                        evidence.get("revision").and_then(Value::as_i64).unwrap_or(0),
                        cause.get("explanation").and_then(Value::as_str).unwrap_or("")
                    ],
                )
                .map_err(|error| error.to_string())?;
            }
        }
        _ => {}
    }
    let _ = data_json;
    replace_periods(tx, record_id, map)?;
    replace_evidence(tx, record_id, map)?;
    Ok(())
}

fn replace_periods(tx: &Transaction<'_>, record_id: &str, map: &Map<String, Value>) -> Result<(), String> {
    tx.execute(
        "DELETE FROM provider_reporting_periods WHERE owner_record_id = ?1",
        [record_id],
    )
    .map_err(|error| error.to_string())?;
    let Some(periods) = map.get("provider_reporting_periods").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, period) in periods.iter().enumerate() {
        let json = serde_json::to_string(period).map_err(|error| error.to_string())?;
        tx.execute(
            "INSERT INTO provider_reporting_periods (
                owner_record_id, period_order, period_type, provider_period_id, provider_date, start_at, end_at, period_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record_id,
                index as i64,
                period.get("period_type").and_then(Value::as_str).unwrap_or(""),
                text_bind(period, "provider_period_id"),
                text_bind(period, "provider_date"),
                text_bind(period, "start_at"),
                text_bind(period, "end_at"),
                json
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn replace_evidence(tx: &Transaction<'_>, record_id: &str, map: &Map<String, Value>) -> Result<(), String> {
    tx.execute(
        "DELETE FROM authored_record_evidence WHERE authored_record_id = ?1",
        [record_id],
    )
    .map_err(|error| error.to_string())?;
    let Some(evidence) = map
        .get("authorship")
        .and_then(|value| value.get("evidence"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    for (index, item) in evidence.iter().enumerate() {
        tx.execute(
            "INSERT INTO authored_record_evidence (
                authored_record_id, evidence_order, evidence_record_id, evidence_revision
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                record_id,
                index as i64,
                item.get("record_id").and_then(Value::as_str).unwrap_or(""),
                item.get("revision").and_then(Value::as_i64).unwrap_or(0)
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn replace_links(tx: &Transaction<'_>, record_id: &str, ids: Option<&Vec<Value>>) -> Result<(), String> {
    tx.execute(
        "DELETE FROM record_attachment_links WHERE owner_record_id = ?1",
        [record_id],
    )
    .map_err(|error| error.to_string())?;
    let Some(ids) = ids else {
        return Ok(());
    };
    for (index, id) in ids.iter().enumerate() {
        tx.execute(
            "INSERT INTO record_attachment_links (owner_record_id, attachment_order, attachment_record_id) VALUES (?1, ?2, ?3)",
            params![record_id, index as i64, id.as_str().unwrap_or("")],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn array<'a>(map: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    map.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn replace_json_children(
    tx: &Transaction<'_>,
    table: &str,
    parent_column: &str,
    order_column: &str,
    json_column: &str,
    parent_id: &str,
    children: &[Value],
) -> Result<(), String> {
    tx.execute(
        &format!("DELETE FROM {table} WHERE {parent_column} = ?1"),
        [parent_id],
    )
    .map_err(|error| error.to_string())?;
    for (index, child) in children.iter().enumerate() {
        let json = serde_json::to_string(child).map_err(|error| error.to_string())?;
        tx.execute(
            &format!("INSERT INTO {table} ({parent_column}, {order_column}, {json_column}) VALUES (?1, ?2, ?3)"),
            params![parent_id, index as i64, json],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn validate(tx: &Transaction<'_>, action: &Value) -> Result<(), String> {
    let record_type = required_str(action, "record_type").or_else(|_| {
        let record_id = required_str(action, "record_id")?;
        tx.query_row(
            "SELECT record_type FROM records WHERE record_id = ?1",
            [&record_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
    })?;
    let data_json = if let Some(json) = action.get("data_json").and_then(Value::as_str) {
        json.to_string()
    } else {
        let record_id = required_str(action, "record_id")?;
        tx.query_row(
            "SELECT record_revisions.record_json FROM records
             JOIN record_revisions ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&record_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    };
    let data: Value = serde_json::from_str(&data_json).map_err(|error| error.to_string())?;
    if record_type == "prediction_result_link" {
        let _ = linked(tx, &data)?;
        return Ok(());
    }
    if record_type != "prediction_grade" {
        return Ok(());
    }
    let link_ref = data.get("prediction_result_link").ok_or("prediction_result_link")?;
    let link = read_record_revision(
        tx,
        link_ref.get("record_id").and_then(Value::as_str).unwrap_or(""),
        link_ref.get("revision").and_then(Value::as_i64).unwrap_or(0),
        "prediction_result_link",
    )?;
    let (prediction, result) = linked(tx, &link)?;
    if data.get("predicted_score") != prediction.get("predicted_score") {
        return Err("Saved grade predicted score does not match the linked prediction revision".into());
    }
    if data.get("actual_score") != result.get("score") {
        return Err("Saved grade actual score does not match the linked result revision".into());
    }
    let definition_id = prediction
        .get("benchmark_definition_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let score_type: Option<String> = tx
        .query_row(
            "SELECT score_type FROM benchmark_definitions WHERE record_id = ?1",
            [definition_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(score_type) = score_type else {
        return Err("Linked benchmark definition does not exist".into());
    };
    let predicted = native_score(prediction.get("predicted_score"), &score_type)?;
    let actual = native_score(result.get("score"), &score_type)?;
    let signed = js_round((predicted - actual) * 100.0) / 100.0;
    let percentage = js_round((signed.abs() / actual) * 1000.0) / 10.0;
    let in_range = prediction
        .get("predicted_range")
        .and_then(|range| {
            let low = native_score(Some(&range["low"]), &score_type).ok()?;
            let high = native_score(Some(&range["high"]), &score_type).ok()?;
            Some(actual >= low && actual <= high)
        })
        .unwrap_or(false);
    let saved_signed = data.get("signed_error").and_then(Value::as_f64);
    let saved_percentage = data.get("absolute_error_percentage").and_then(Value::as_f64);
    let saved_range = data.get("result_in_predicted_range").and_then(Value::as_bool);
    if saved_signed != Some(signed) || saved_percentage != Some(percentage) || saved_range != Some(in_range) {
        return Err("Saved grade arithmetic does not match its linked revisions".into());
    }
    Ok(())
}

fn linked(tx: &Transaction<'_>, link: &Value) -> Result<(Value, Value), String> {
    let prediction_ref = link.get("prediction").ok_or("prediction")?;
    let result_ref = link.get("benchmark_result").ok_or("benchmark_result")?;
    let prediction = read_record_revision(
        tx,
        prediction_ref.get("record_id").and_then(Value::as_str).unwrap_or(""),
        prediction_ref.get("revision").and_then(Value::as_i64).unwrap_or(0),
        "prediction",
    )?;
    let result = read_record_revision(
        tx,
        result_ref.get("record_id").and_then(Value::as_str).unwrap_or(""),
        result_ref.get("revision").and_then(Value::as_i64).unwrap_or(0),
        "benchmark_result",
    )?;
    if prediction.get("benchmark_definition_id") != result.get("benchmark_definition_id") {
        return Err("Prediction and benchmark result use different benchmark definitions".into());
    }
    Ok((prediction, result))
}

fn read_record_revision(tx: &Transaction<'_>, record_id: &str, revision: i64, expected: &str) -> Result<Value, String> {
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT record_type, record_json FROM record_revisions WHERE record_id = ?1 AND revision = ?2",
            params![record_id, revision],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((record_type, json)) = row else {
        return Err(format!("Referenced record revision does not exist: {record_id}@{revision}"));
    };
    if record_type != expected {
        return Err(format!("Referenced {record_id}@{revision} is {record_type}, not {expected}"));
    }
    serde_json::from_str(&json).map_err(|error| error.to_string())
}

fn native_score(score: Option<&Value>, score_type: &str) -> Result<f64, String> {
    let score = score.ok_or("score")?;
    let key = match score_type {
        "time" => "duration_s",
        "reps" => "reps",
        _ => "weight_kg",
    };
    score
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("Score does not contain the benchmark's {score_type} value"))
}

fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn typed_table(record_type: &str) -> Result<&'static str, String> {
    Ok(match record_type {
        "athlete_profile" => "athlete_profile",
        "source" => "sources",
        "source_device" => "source_devices",
        "point_measurement" => "point_measurements",
        "measurement_sample" => "measurement_samples",
        "sleep_session" => "sleep_sessions",
        "workout_session" => "workout_sessions",
        "benchmark_result" => "benchmark_results",
        "provider_summary" => "provider_summaries",
        "provider_score" => "provider_scores",
        "athlete_report" => "athlete_reports",
        "nutrition_entry" => "nutrition_entries",
        "attachment" => "attachments",
        "benchmark_definition" => "benchmark_definitions",
        "prediction" => "predictions",
        "prediction_result_link" => "prediction_result_links",
        "prediction_grade" => "prediction_grades",
        "prediction_error_analysis" => "prediction_error_analyses",
        "training_plan" => "training_plans",
        "planned_session" => "planned_sessions",
        "coaching_note" => "coaching_notes",
        "saved_training_review" => "saved_training_reviews",
        "goal" => "goals",
        "nutrition_model_interpretation" => "nutrition_model_interpretations",
        other => return Err(format!("record type {other} has no typed table")),
    })
}

fn typed_fields(record_type: &str, data: &Map<String, Value>) -> Result<Vec<(String, Bind)>, String> {
    let text = |key: &str| bind_text(data.get(key));
    let author = data.get("authorship");
    let author_at = author.and_then(|value| value.get("authored_at"));
    let author_type = author.and_then(|value| value.get("author")).and_then(|value| value.get("author_type"));
    let content_version = author.and_then(|value| value.get("content_version"));
    let status = author.and_then(|value| value.get("status"));
    let value_obj = data.get("value");
    Ok(match record_type {
        "athlete_profile" => vec![("default_time_zone".into(), text("default_time_zone"))],
        "source" => vec![
            ("source_type".into(), text("source_type")),
            ("data_writer".into(), text("data_writer")),
            ("data_route".into(), text("data_route")),
            ("vendor".into(), text("vendor")),
            ("sensor_type".into(), text("sensor_type")),
        ],
        "source_device" => vec![
            ("source_id".into(), text("source_id")),
            ("name".into(), text("name")),
            ("manufacturer".into(), text("manufacturer")),
            ("model".into(), text("model")),
            ("hardware".into(), text("hardware")),
            ("serial_number".into(), text("serial_number")),
            ("coverage_start_date".into(), text("coverage_start_date")),
            ("coverage_end_date".into(), text("coverage_end_date")),
            ("accepted_record_count".into(), bind_number(data.get("accepted_record_count"))),
        ],
        "point_measurement" | "measurement_sample" => {
            let mut fields = vec![
                ("source_id".into(), text("source_id")),
                ("quantity".into(), text("quantity")),
                ("recorded_at".into(), text("recorded_at")),
                ("standard_value".into(), bind_number(value_obj.and_then(|value| value.get("standard_value")))),
                ("standard_unit".into(), bind_text(value_obj.and_then(|value| value.get("standard_unit")))),
            ];
            if record_type == "measurement_sample" {
                fields.push(("duration_ms".into(), bind_number(data.get("duration_ms"))));
            }
            fields.push(("provider_record_id".into(), text("provider_record_id")));
            fields.push(("received_at".into(), text("received_at")));
            fields
        }
        "sleep_session" => vec![
            ("source_id".into(), text("source_id")),
            ("start_at".into(), text("start_at")),
            ("end_at".into(), text("end_at")),
            ("provider_record_id".into(), text("provider_record_id")),
            ("received_at".into(), text("received_at")),
        ],
        "workout_session" => vec![
            ("source_id".into(), text("source_id")),
            ("start_at".into(), text("start_at")),
            ("end_at".into(), text("end_at")),
            ("activity_type".into(), text("activity_type")),
            ("name".into(), text("name")),
            ("description".into(), text("description")),
            ("text_result".into(), text("text_result")),
            ("provider_record_id".into(), text("provider_record_id")),
            ("received_at".into(), text("received_at")),
        ],
        "benchmark_result" => vec![
            ("source_id".into(), text("source_id")),
            ("benchmark_definition_id".into(), text("benchmark_definition_id")),
            ("recorded_at".into(), text("recorded_at")),
            ("workout_session_id".into(), text("workout_session_id")),
            ("provider_record_id".into(), text("provider_record_id")),
            ("received_at".into(), text("received_at")),
        ],
        "provider_score" => vec![
            ("source_id".into(), text("source_id")),
            ("metric".into(), text("metric")),
            ("recorded_at".into(), text("recorded_at")),
            ("standard_value".into(), bind_number(value_obj.and_then(|value| value.get("standard_value")))),
            ("standard_unit".into(), bind_text(value_obj.and_then(|value| value.get("standard_unit")))),
            ("scale".into(), text("scale")),
            ("provider_record_id".into(), text("provider_record_id")),
            ("received_at".into(), text("received_at")),
        ],
        "provider_summary" => vec![
            ("source_id".into(), text("source_id")),
            ("summary_type".into(), text("summary_type")),
            ("recorded_at".into(), text("recorded_at")),
            ("standard_value".into(), bind_number(value_obj.and_then(|value| value.get("standard_value")))),
            ("standard_unit".into(), bind_text(value_obj.and_then(|value| value.get("standard_unit")))),
            ("provider_record_id".into(), text("provider_record_id")),
            ("received_at".into(), text("received_at")),
        ],
        "athlete_report" => vec![
            ("report_type".into(), text("report_type")),
            ("reported_at".into(), text("reported_at")),
        ],
        "nutrition_entry" => vec![
            ("reported_at".into(), text("reported_at")),
            ("athlete_description".into(), text("athlete_description")),
        ],
        "attachment" => vec![
            ("file_name".into(), text("file_name")),
            ("media_type".into(), text("media_type")),
            ("byte_count".into(), bind_number(data.get("byte_count"))),
            ("sha256".into(), text("sha256")),
        ],
        "benchmark_definition" => vec![
            ("benchmark_key".into(), text("benchmark_key")),
            ("benchmark_type".into(), text("benchmark_type")),
            ("score_type".into(), text("score_type")),
            ("definition".into(), text("definition")),
        ],
        "prediction" | "training_plan" | "coaching_note" | "saved_training_review" | "nutrition_model_interpretation" => {
            let mut fields = Vec::new();
            if record_type == "prediction" {
                fields.push(("benchmark_definition_id".into(), text("benchmark_definition_id")));
            }
            if record_type == "training_plan" {
                fields.push(("title".into(), text("title")));
                fields.push(("start_date".into(), text("start_date")));
                fields.push(("end_date".into(), text("end_date")));
            }
            if record_type == "saved_training_review" {
                fields.push(("period_start".into(), text("period_start")));
                fields.push(("period_end".into(), text("period_end")));
            }
            if record_type == "nutrition_model_interpretation" {
                fields.push(("nutrition_entry_id".into(), text("nutrition_entry_id")));
                fields.push(("confidence".into(), text("confidence")));
            }
            fields.push(("authored_at".into(), bind_text(author_at)));
            fields.push(("author_type".into(), bind_text(author_type)));
            fields.push(("content_version".into(), bind_text(content_version)));
            fields.push(("status".into(), bind_text(status)));
            fields
        }
        "prediction_result_link" => vec![
            ("prediction_id".into(), bind_text(data.get("prediction").and_then(|value| value.get("record_id")))),
            ("prediction_revision".into(), bind_number(data.get("prediction").and_then(|value| value.get("revision")))),
            ("benchmark_result_id".into(), bind_text(data.get("benchmark_result").and_then(|value| value.get("record_id")))),
            ("benchmark_result_revision".into(), bind_number(data.get("benchmark_result").and_then(|value| value.get("revision")))),
            ("linked_at".into(), text("linked_at")),
            ("created_by_type".into(), bind_text(data.get("created_by").and_then(|value| value.get("author_type")))),
            ("content_version".into(), text("content_version")),
            ("status".into(), text("status")),
        ],
        "prediction_grade" => vec![
            ("prediction_result_link_id".into(), bind_text(data.get("prediction_result_link").and_then(|value| value.get("record_id")))),
            ("prediction_result_link_revision".into(), bind_number(data.get("prediction_result_link").and_then(|value| value.get("revision")))),
            ("authored_at".into(), bind_text(author_at)),
            ("author_type".into(), bind_text(author_type)),
            ("content_version".into(), bind_text(content_version)),
            ("status".into(), bind_text(status)),
            ("signed_error".into(), bind_number(data.get("signed_error"))),
            ("absolute_error_percentage".into(), bind_number(data.get("absolute_error_percentage"))),
            ("result_in_predicted_range".into(), bind_bool(data.get("result_in_predicted_range"))),
        ],
        "prediction_error_analysis" => vec![
            ("prediction_grade_id".into(), bind_text(data.get("prediction_grade").and_then(|value| value.get("record_id")))),
            ("prediction_grade_revision".into(), bind_number(data.get("prediction_grade").and_then(|value| value.get("revision")))),
            ("authored_at".into(), bind_text(author_at)),
            ("author_type".into(), bind_text(author_type)),
            ("content_version".into(), bind_text(content_version)),
            ("status".into(), bind_text(status)),
        ],
        "planned_session" => vec![
            ("training_plan_id".into(), text("training_plan_id")),
            ("title".into(), text("title")),
            ("scheduled_date".into(), text("scheduled_date")),
            ("scheduled_start_at".into(), text("scheduled_start_at")),
            ("completion_status".into(), text("completion_status")),
            ("completed_workout_session_id".into(), text("completed_workout_session_id")),
            ("authored_at".into(), bind_text(author_at)),
            ("author_type".into(), bind_text(author_type)),
            ("content_version".into(), bind_text(content_version)),
            ("status".into(), bind_text(status)),
        ],
        "goal" => vec![
            ("title".into(), text("title")),
            ("target_date".into(), text("target_date")),
            ("target_benchmark_definition_id".into(), text("target_benchmark_definition_id")),
            ("authored_at".into(), bind_text(author_at)),
            ("author_type".into(), bind_text(author_type)),
            ("content_version".into(), bind_text(content_version)),
            ("status".into(), bind_text(status)),
        ],
        other => return Err(format!("record type {other} has no typed fields")),
    })
}

fn required_str(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing {key}"))
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn text_bind(value: &Value, key: &str) -> Bind {
    bind_text(value.get(key))
}

fn bind_text(value: Option<&Value>) -> Bind {
    match value {
        Some(Value::String(text)) => Bind::Text(text.clone()),
        Some(Value::Number(number)) => Bind::Text(number.to_string()),
        _ => Bind::Null,
    }
}

fn bind_number(value: Option<&Value>) -> Bind {
    match value {
        Some(Value::Number(number)) => {
            if let Some(integer) = number.as_i64() {
                Bind::I64(integer)
            } else if let Some(real) = number.as_f64() {
                Bind::F64(real)
            } else {
                Bind::Null
            }
        }
        _ => Bind::Null,
    }
}

fn bind_bool(value: Option<&Value>) -> Bind {
    match value {
        Some(Value::Bool(true)) => Bind::I64(1),
        Some(Value::Bool(false)) => Bind::I64(0),
        _ => Bind::Null,
    }
}

fn parse_millis(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| time.timestamp_millis())
        .unwrap_or(0)
}

#[derive(Clone)]
enum Bind {
    Null,
    I64(i64),
    F64(f64),
    Text(String),
}

impl rusqlite::types::ToSql for Bind {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Bind::Null => Ok(rusqlite::types::ToSqlOutput::Owned(rusqlite::types::Value::Null)),
            Bind::I64(value) => Ok(rusqlite::types::ToSqlOutput::Owned(rusqlite::types::Value::Integer(*value))),
            Bind::F64(value) => Ok(rusqlite::types::ToSqlOutput::Owned(rusqlite::types::Value::Real(*value))),
            Bind::Text(value) => Ok(rusqlite::types::ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(value.as_bytes()))),
        }
    }
}

fn binds_to_params(binds: &[Bind]) -> Vec<Bind> {
    binds.to_vec()
}

#[cfg(test)]
mod tests {
    use super::apply;
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn a_seventeen_digit_number_keeps_its_bits() {
        let text = "14.384010205470487";
        let parsed: f64 = serde_json::from_str(text).unwrap();
        let exact: f64 = text.parse().unwrap();
        assert_eq!(parsed.to_bits(), exact.to_bits());
        let printed = serde_json::to_string(&parsed).unwrap();
        let again: f64 = serde_json::from_str(&printed).unwrap();
        assert_eq!(again.to_bits(), exact.to_bits());
    }

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../../spec/sql/migrations/001-initial.sql")).unwrap();
        conn.execute(
            "INSERT INTO database_metadata (singleton_id, application_marker, sql_schema_version, athleticstandard_version, database_id, created_at) VALUES (1, 'ATH1', 3, '2.0.0', '018f0000-0000-7000-8000-000000000099', '2026-08-09T00:00:00.000Z')",
            [],
        ).unwrap();
        conn.execute("INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0)", []).unwrap();
        conn.execute_batch(include_str!("../../../spec/sql/migrations/003-version-2.sql")).unwrap();
        conn
    }

    #[test]
    fn persist_inserts_a_source_and_bumps_the_revision() {
        let mut conn = database();
        let result = apply(&mut conn, &json!({
            "actions": [
                {
                    "kind": "insert",
                    "record_id": "018f0000-0000-7000-8000-0000000000a1",
                    "record_type": "source",
                    "created_at": "2026-08-09T00:00:00.000Z",
                    "data_json": "{\"source_type\":\"wearable\",\"data_writer\":\"Oura\",\"data_route\":\"oura_csv\",\"vendor\":\"oura\"}",
                    "extensions_json": null,
                    "changed_by_json": "{\"author_type\":\"tool\",\"tool\":\"ath-core\",\"tool_version\":\"2.1.0\"}",
                    "became_known_at": "2026-08-09T00:00:00.000Z",
                    "became_known_at_basis": "provider_import"
                },
                { "kind": "validate" },
                { "kind": "bump" }
            ]
        })).unwrap();
        assert_eq!(result["database_revision"], 1);
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
