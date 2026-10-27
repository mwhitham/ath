//! The eight range calculations. The skill file names the calculation.
//! This module reads the stored rows and does the arithmetic.

use chrono::{Datelike, DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

const MAX_INPUTS: i64 = 100_000;
const MAX_REFS: usize = 1_000;

struct Input {
    record_id: String,
    revision: i64,
    source_id: String,
    recorded_at: String,
    value: f64,
    unit: String,
    period: Option<Value>,
}

pub fn calculate(conn: &Connection, options: &Value) -> Result<Value, String> {
    let operation = req(options, "operation")?;
    let quantity = req(options, "quantity")?;
    let record_kind = options.get("record_kind").and_then(Value::as_str).unwrap_or("measurement");
    let method = options.get("method").and_then(Value::as_str).unwrap_or("").to_string();
    if operation != "provider-period-summary" && record_kind == "session-summary" {
        return Err("session-summary records require the provider-period-summary operation".into());
    }
    if operation == "provider-period-summary" {
        let period_type = req(options, "provider_period_type")?;
        let inputs = read_period(conn, &quantity, record_kind, &period_type, options)?;
        if inputs.is_empty() {
            return Err(format!("No current {record_kind} {quantity} records matched {period_type}"));
        }
        let unit = one_unit(&inputs, None)?;
        return Ok(receipt(options, &unit, &inputs, json_range(options), &method_or(&method, "count, minimum, maximum, mean, and sum per exact provider period and source; no calendar boundary conversion"), period_summaries(&inputs), &["manual sources excluded", &format!("records without {period_type} excluded"), "provider periods were not converted to calendar days"]));
    }
    if options.get("provider_period_type").and_then(Value::as_str).is_some() {
        return Err("--period-type is used only with provider-period-summary".into());
    }
    let expected = options.get("expected_unit").and_then(Value::as_str);
    if operation == "workout-drift" {
        return workout_drift(conn, options, &quantity, record_kind, expected, &method);
    }
    let inputs = read_inputs(conn, &quantity, record_kind, options)?;
    if inputs.is_empty() {
        return Err(format!(
            "No current {record_kind} {quantity} records with an instant matched. Date-only records require provider-period-summary."
        ));
    }
    let unit = one_unit(&inputs, expected)?;
    let group = options.get("group").and_then(Value::as_str).unwrap_or("none");
    match operation.as_str() {
        "summary" | "baseline" => {
            let mut result = Vec::new();
            for (source_id, values) in by_source(&inputs) {
                let numbers: Vec<f64> = values.iter().map(|input| input.value).collect();
                let mut object = json!({ "source_id": source_id });
                if group == "none" {
                    merge(&mut object, &summarize(&numbers));
                } else {
                    let zone = req(options, "time_zone")?;
                    object["groups"] = json!(grouped(&values, group, &zone)?);
                }
                result.push(object);
            }
            let method_text = if operation == "baseline" {
                "population mean and population standard deviation per source"
            } else if group == "none" {
                "count, minimum, maximum, mean, and sum per source"
            } else {
                return Ok(receipt(options, &unit, &inputs, json_range(options), &format!("{group} calendar groups per source"), json!(result), &["manual sources excluded"]));
            };
            Ok(receipt(options, &unit, &inputs, json_range(options), &method_or(&method, method_text), json!(result), &["manual sources excluded"]))
        }
        "trend" => {
            let result = by_source(&inputs)
                .into_iter()
                .map(|(source_id, values)| {
                    let mut object = json!({ "source_id": source_id });
                    let points: Vec<(String, f64)> = values.iter().map(|input| (input.recorded_at.clone(), input.value)).collect();
                    merge(&mut object, &trend(&points)?);
                    Ok(object)
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(receipt(options, &unit, &inputs, json_range(options), &method_or(&method, "ordinary least-squares slope against elapsed days, calculated separately per source"), json!(result), &["manual sources excluded"]))
        }
        "coverage" => coverage(options, &unit, &inputs),
        "compare-sources" => compare_sources(conn, options, &quantity, record_kind, expected, &unit, inputs),
        "correlation" => correlate(conn, options, record_kind, &unit, inputs),
        other => Err(format!("Unsupported calculation operation: {other}")),
    }
}

fn workout_drift(
    conn: &Connection,
    options: &Value,
    quantity: &str,
    record_kind: &str,
    expected: Option<&str>,
    method: &str,
) -> Result<Value, String> {
    let session_id = req(options, "workout_session_id")?;
    let row = conn
        .query_row(
            "SELECT records.record_type, record_revisions.record_json, records.is_deleted
             FROM records
             JOIN record_revisions ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [&session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .filter(|(_, _, deleted)| *deleted == 0)
        .map(|(kind, json, _)| (kind, json));
    let Some((record_type, json)) = row else {
        return Err(format!("{session_id} is not a current workout session"));
    };
    if record_type != "workout_session" {
        return Err(format!("{session_id} is not a current workout session"));
    }
    let data: Value = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let start = data.get("start_at").and_then(Value::as_str).unwrap_or("").to_string();
    let end = data.get("end_at").and_then(Value::as_str).unwrap_or("").to_string();
    let mut window = options.clone();
    window["from"] = json!(start);
    window["to"] = json!(end);
    let inputs = read_inputs(conn, quantity, record_kind, &window)?;
    if inputs.len() < 2 {
        return Err("workout-drift needs at least two samples inside the session".into());
    }
    let unit = one_unit(&inputs, expected)?;
    let start_ms = parse_ms(&start);
    let end_ms = parse_ms(&end);
    let midpoint = start_ms + (end_ms - start_ms) / 2;
    let sources = by_source(&inputs)
        .into_iter()
        .map(|(source_id, values)| {
            let first: Vec<f64> = values.iter().filter(|input| parse_ms(&input.recorded_at) < midpoint).map(|input| input.value).collect();
            let second: Vec<f64> = values.iter().filter(|input| parse_ms(&input.recorded_at) >= midpoint).map(|input| input.value).collect();
            if first.is_empty() || second.is_empty() {
                return json!({ "source_id": source_id, "available": false, "reason": "one half has no samples" });
            }
            let first_mean = summarize(&first)["mean"].as_f64().unwrap_or(0.0);
            let second_mean = summarize(&second)["mean"].as_f64().unwrap_or(0.0);
            json!({
                "source_id": source_id,
                "available": true,
                "first_half_mean": first_mean,
                "second_half_mean": second_mean,
                "change": round4(second_mean - first_mean),
                "change_percentage": round4(((second_mean - first_mean) / first_mean) * 100.0),
            })
        })
        .collect::<Vec<_>>();
    let mut range = json!({});
    range["from"] = json!(start);
    range["to"] = json!(end);
    Ok(receipt(
        options,
        &unit,
        &inputs,
        range,
        &method_or(method, "difference between means in equal-duration first and second workout halves"),
        json!({ "workout_session_id": session_id, "sources": sources }),
        &["manual sources excluded", "samples outside the workout session excluded"],
    ))
}

fn coverage(options: &Value, unit: &str, inputs: &[Input]) -> Result<Value, String> {
    let zone = req(options, "time_zone")?;
    let result = by_source(inputs)
        .into_iter()
        .map(|(source_id, values)| {
            let mut present = values
                .iter()
                .map(|input| calendar_date(&input.recorded_at, &zone))
                .collect::<Result<Vec<_>, _>>()?;
            present.sort();
            present.dedup();
            let from = options.get("calendar_from").or_else(|| options.get("from")).and_then(Value::as_str).map(|value| value.chars().take(10).collect::<String>());
            let to = options.get("calendar_to").or_else(|| options.get("to")).and_then(Value::as_str).map(|value| value.chars().take(10).collect::<String>());
            let start = from.unwrap_or_else(|| present.first().cloned().unwrap_or_default());
            let end = to.unwrap_or_else(|| present.last().cloned().unwrap_or_default());
            let expected = calendar_dates(&start, &end);
            let present_set = present.iter().cloned().collect::<std::collections::BTreeSet<_>>();
            let missing = expected.iter().filter(|date| !present_set.contains(*date)).cloned().collect::<Vec<_>>();
            Ok(json!({
                "source_id": source_id,
                "first_date": expected.first(),
                "last_date": expected.last(),
                "days_expected": expected.len(),
                "days_present": present.len(),
                "missing_dates": missing,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(receipt(options, unit, inputs, json_range(options), "calendar dates containing at least one record, calculated separately per source", json!(result), &["manual sources excluded"]))
}

fn compare_sources(
    conn: &Connection,
    options: &Value,
    quantity: &str,
    record_kind: &str,
    expected: Option<&str>,
    unit: &str,
    mut inputs: Vec<Input>,
) -> Result<Value, String> {
    let source_id = req(options, "source_id")?;
    let compare_id = req(options, "compare_source_id")?;
    let mut other_options = options.clone();
    other_options["source_id"] = json!(compare_id);
    let other = read_inputs(conn, quantity, record_kind, &other_options)?;
    if other.is_empty() {
        return Err(format!("No {quantity} records matched --compare-source"));
    }
    one_unit(&other, Some(unit)).or_else(|_| one_unit(&other, expected))?;
    let left = summarize(&inputs.iter().map(|input| input.value).collect::<Vec<_>>());
    let right = summarize(&other.iter().map(|input| input.value).collect::<Vec<_>>());
    inputs.extend(other);
    let difference = round4(left["mean"].as_f64().unwrap_or(0.0) - right["mean"].as_f64().unwrap_or(0.0));
    Ok(receipt(
        options,
        unit,
        &inputs,
        json_range(options),
        "separate source summaries and difference between their means",
        json!({
            "source": { "source_id": source_id, "count": left["count"], "minimum": left["minimum"], "maximum": left["maximum"], "mean": left["mean"], "sum": left["sum"], "standard_deviation": left["standard_deviation"] },
            "comparison_source": { "source_id": compare_id, "count": right["count"], "minimum": right["minimum"], "maximum": right["maximum"], "mean": right["mean"], "sum": right["sum"], "standard_deviation": right["standard_deviation"] },
            "mean_difference": difference,
        }),
        &["manual sources excluded", "records are not paired by timestamp"],
    ))
}

fn correlate(
    conn: &Connection,
    options: &Value,
    record_kind: &str,
    unit: &str,
    mut inputs: Vec<Input>,
) -> Result<Value, String> {
    let other_quantity = req(options, "other_quantity")?;
    let zone = req(options, "time_zone")?;
    let mut other_options = options.clone();
    if let Some(compare) = options.get("compare_source_id").and_then(Value::as_str) {
        other_options["source_id"] = json!(compare);
    }
    let other = read_inputs(conn, &other_quantity, record_kind, &other_options)?;
    one_unit(&other, options.get("expected_other_unit").and_then(Value::as_str))?;
    let left = daily_means(&inputs, &zone)?;
    let right = daily_means(&other, &zone)?;
    let dates = left.keys().filter(|date| right.contains_key(*date)).cloned().collect::<Vec<_>>();
    inputs.extend(other);
    let correlation = pearson(
        &dates.iter().map(|date| left[date]).collect::<Vec<_>>(),
        &dates.iter().map(|date| right[date]).collect::<Vec<_>>(),
    );
    Ok(receipt(
        options,
        unit,
        &inputs,
        json_range(options),
        "Pearson correlation of same-calendar-day means",
        json!({
            "other_quantity": other_quantity,
            "paired_day_count": dates.len(),
            "correlation": correlation,
            "paired_dates": dates,
        }),
        &["manual sources excluded", "days missing either quantity excluded"],
    ))
}

fn read_inputs(conn: &Connection, quantity: &str, record_kind: &str, options: &Value) -> Result<Vec<Input>, String> {
    let (values_sql, quantity_args) = match record_kind {
        "provider-score" => (
            "SELECT provider_scores.record_id, provider_scores.source_id, provider_scores.recorded_at, provider_scores.standard_value, provider_scores.standard_unit FROM provider_scores WHERE provider_scores.metric = ?1 AND provider_scores.recorded_at IS NOT NULL".to_string(),
            vec![quantity.to_string()],
        ),
        "provider-summary" => (
            "SELECT provider_summaries.record_id, provider_summaries.source_id, provider_summaries.recorded_at, provider_summaries.standard_value, provider_summaries.standard_unit FROM provider_summaries WHERE provider_summaries.summary_type = ?1 AND provider_summaries.recorded_at IS NOT NULL".to_string(),
            vec![quantity.to_string()],
        ),
        _ => (
            "SELECT point_measurements.record_id, point_measurements.source_id, point_measurements.recorded_at, point_measurements.standard_value, point_measurements.standard_unit FROM point_measurements WHERE point_measurements.quantity = ?1 AND point_measurements.recorded_at IS NOT NULL UNION ALL SELECT measurement_samples.record_id, measurement_samples.source_id, measurement_samples.recorded_at, measurement_samples.standard_value, measurement_samples.standard_unit FROM measurement_samples WHERE measurement_samples.quantity = ?2".to_string(),
            vec![quantity.to_string(), quantity.to_string()],
        ),
    };
    let mut conditions = vec!["records.is_deleted = 0".to_string(), "sources.source_type <> 'manual'".to_string()];
    let mut args = quantity_args;
    if let Some(source) = options.get("source_id").and_then(Value::as_str) {
        conditions.push("values_by_record.source_id = ?".into());
        args.push(source.into());
    }
    if let Some(from) = options.get("from").and_then(Value::as_str) {
        conditions.push("values_by_record.recorded_at >= ?".into());
        args.push(from.into());
    }
    if let Some(to) = options.get("to").and_then(Value::as_str) {
        conditions.push("values_by_record.recorded_at <= ?".into());
        args.push(to.into());
    }
    args.push((MAX_INPUTS + 1).to_string());
    let sql = format!(
        "WITH values_by_record AS ({values_sql})
         SELECT values_by_record.record_id, records.current_revision, values_by_record.source_id,
                values_by_record.recorded_at, values_by_record.standard_value, values_by_record.standard_unit
         FROM values_by_record
         JOIN records ON records.record_id = values_by_record.record_id
         JOIN sources ON sources.record_id = values_by_record.source_id
         WHERE {}
         ORDER BY values_by_record.source_id, values_by_record.recorded_at, values_by_record.record_id
         LIMIT ?",
        conditions.join(" AND ")
    );
    query_inputs(conn, &sql, &args)
}

fn read_period(
    conn: &Connection,
    quantity: &str,
    record_kind: &str,
    period_type: &str,
    options: &Value,
) -> Result<Vec<Input>, String> {
    let (values_sql, mut args) = match record_kind {
        "provider-score" => (
            "SELECT provider_scores.record_id, provider_scores.source_id, provider_scores.standard_value, provider_scores.standard_unit FROM provider_scores WHERE provider_scores.metric = ?1".to_string(),
            vec![quantity.to_string()],
        ),
        "provider-summary" => (
            "SELECT provider_summaries.record_id, provider_summaries.source_id, provider_summaries.standard_value, provider_summaries.standard_unit FROM provider_summaries WHERE provider_summaries.summary_type = ?1".to_string(),
            vec![quantity.to_string()],
        ),
        "session-summary" => (
            "SELECT sleep_sessions.record_id, sleep_sessions.source_id, CAST(json_extract(summary.value, '$.standard_value') AS REAL) AS standard_value, CAST(json_extract(summary.value, '$.standard_unit') AS TEXT) AS standard_unit FROM sleep_sessions JOIN json_each(sleep_sessions.data_json, '$.source_reported_summaries') AS summary WHERE json_extract(summary.value, '$.summary_type') = ?1 UNION ALL SELECT workout_sessions.record_id, workout_sessions.source_id, CAST(json_extract(summary.value, '$.standard_value') AS REAL), CAST(json_extract(summary.value, '$.standard_unit') AS TEXT) FROM workout_sessions JOIN json_each(workout_sessions.data_json, '$.source_reported_summaries') AS summary WHERE json_extract(summary.value, '$.summary_type') = ?2".to_string(),
            vec![quantity.to_string(), quantity.to_string()],
        ),
        _ => (
            "SELECT point_measurements.record_id, point_measurements.source_id, point_measurements.standard_value, point_measurements.standard_unit FROM point_measurements WHERE point_measurements.quantity = ?1 UNION ALL SELECT measurement_samples.record_id, measurement_samples.source_id, measurement_samples.standard_value, measurement_samples.standard_unit FROM measurement_samples WHERE measurement_samples.quantity = ?2".to_string(),
            vec![quantity.to_string(), quantity.to_string()],
        ),
    };
    let mut conditions = vec![
        "records.is_deleted = 0".to_string(),
        "sources.source_type <> 'manual'".to_string(),
        "provider_reporting_periods.period_type = ?".to_string(),
    ];
    args.push(period_type.into());
    if let Some(source) = options.get("source_id").and_then(Value::as_str) {
        conditions.push("values_by_record.source_id = ?".into());
        args.push(source.into());
    }
    if let Some(from) = options.get("from").and_then(Value::as_str) {
        conditions.push("COALESCE(provider_reporting_periods.provider_date, provider_reporting_periods.start_at) >= ?".into());
        args.push(from.into());
    }
    if let Some(to) = options.get("to").and_then(Value::as_str) {
        conditions.push("COALESCE(provider_reporting_periods.provider_date, provider_reporting_periods.start_at) <= ?".into());
        args.push(to.into());
    }
    args.push((MAX_INPUTS + 1).to_string());
    let sql = format!(
        "WITH values_by_record AS ({values_sql})
         SELECT values_by_record.record_id, records.current_revision, values_by_record.source_id,
                values_by_record.standard_value, values_by_record.standard_unit, provider_reporting_periods.period_json
         FROM values_by_record
         JOIN records ON records.record_id = values_by_record.record_id
         JOIN sources ON sources.record_id = values_by_record.source_id
         JOIN provider_reporting_periods ON provider_reporting_periods.owner_record_id = values_by_record.record_id
         WHERE {}
         ORDER BY values_by_record.source_id, COALESCE(provider_reporting_periods.provider_date, provider_reporting_periods.start_at), values_by_record.record_id
         LIMIT ?",
        conditions.join(" AND ")
    );
    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(args.iter()), |row| {
            let period_json: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                period_json,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if rows.len() as i64 > MAX_INPUTS {
        return Err(format!("Calculation matched more than {MAX_INPUTS} records. Narrow --from and --to."));
    }
    rows.into_iter()
        .map(|(record_id, revision, source_id, value, unit, period_json)| {
            let period: Value = serde_json::from_str(&period_json).map_err(|error| error.to_string())?;
            let recorded_at = period
                .get("provider_date")
                .and_then(Value::as_str)
                .or_else(|| period.get("start_at").and_then(Value::as_str))
                .or_else(|| period.get("provider_period_id").and_then(Value::as_str))
                .unwrap_or("")
                .to_string();
            Ok(Input { record_id, revision, source_id, recorded_at, value, unit, period: Some(period) })
        })
        .collect()
}

fn query_inputs(conn: &Connection, sql: &str, args: &[String]) -> Result<Vec<Input>, String> {
    let mut statement = conn.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(args.iter()), |row| {
            Ok(Input {
                record_id: row.get(0)?,
                revision: row.get(1)?,
                source_id: row.get(2)?,
                recorded_at: row.get(3)?,
                value: row.get(4)?,
                unit: row.get(5)?,
                period: None,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if rows.len() as i64 > MAX_INPUTS {
        return Err(format!("Calculation matched more than {MAX_INPUTS} records. Narrow --from and --to."));
    }
    Ok(rows)
}

fn summarize(values: &[f64]) -> Value {
    let count = values.len() as f64;
    let sum = values.iter().sum::<f64>();
    let mean = sum / count;
    let variance = values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / count;
    json!({
        "count": values.len(),
        "minimum": values.iter().copied().fold(f64::INFINITY, f64::min),
        "maximum": values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "mean": round4(mean),
        "sum": round4(sum),
        "standard_deviation": round4(variance.sqrt()),
    })
}

fn trend(inputs: &[(String, f64)]) -> Result<Value, String> {
    if inputs.len() < 2 {
        return Err("A trend needs at least two inputs".into());
    }
    let start = parse_ms(&inputs[0].0);
    let points: Vec<(f64, f64)> = inputs
        .iter()
        .map(|input| ((parse_ms(&input.0) - start) as f64 / 86_400_000.0, input.1))
        .collect();
    let x_mean = points.iter().map(|point| point.0).sum::<f64>() / points.len() as f64;
    let y_mean = points.iter().map(|point| point.1).sum::<f64>() / points.len() as f64;
    let numerator = points.iter().map(|point| (point.0 - x_mean) * (point.1 - y_mean)).sum::<f64>();
    let denominator = points.iter().map(|point| (point.0 - x_mean).powi(2)).sum::<f64>();
    if denominator == 0.0 {
        return Err("A trend needs inputs at more than one instant".into());
    }
    Ok(json!({
        "count": points.len(),
        "slope_per_day": round4(numerator / denominator),
        "first_value": inputs[0].1,
        "last_value": inputs.last().unwrap().1,
    }))
}

fn period_summaries(inputs: &[Input]) -> Value {
    let mut sources: Vec<(String, Vec<(String, Value, Vec<f64>)>)> = Vec::new();
    for input in inputs {
        let period = input.period.clone().unwrap_or(Value::Null);
        let key = serde_json::to_string(&json!([
            period.get("provider_period_id").cloned().unwrap_or(Value::Null),
            period.get("provider_date").cloned().unwrap_or(Value::Null),
            period.get("start_at").cloned().unwrap_or(Value::Null),
            period.get("end_at").cloned().unwrap_or(Value::Null),
        ])).unwrap_or_default();
        if let Some(group) = sources.iter_mut().find(|group| group.0 == input.source_id) {
            if let Some(period_group) = group.1.iter_mut().find(|item| item.0 == key) {
                period_group.2.push(input.value);
            } else {
                group.1.push((key, period, vec![input.value]));
            }
        } else {
            sources.push((input.source_id.clone(), vec![(key, period, vec![input.value])]));
        }
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    json!(sources.into_iter().map(|(source_id, mut periods)| {
        periods.sort_by(|left, right| period_sort(&left.1).cmp(&period_sort(&right.1)));
        json!({
            "source_id": source_id,
            "periods": periods.into_iter().map(|(_, period, values)| {
                let mut object = json!({ "provider_period": period });
                merge(&mut object, &summarize(&values));
                object
            }).collect::<Vec<_>>()
        })
    }).collect::<Vec<_>>())
}

fn by_source(inputs: &[Input]) -> Vec<(String, Vec<&Input>)> {
    let mut groups: Vec<(String, Vec<&Input>)> = Vec::new();
    for input in inputs {
        if let Some(group) = groups.iter_mut().find(|group| group.0 == input.source_id) {
            group.1.push(input);
        } else {
            groups.push((input.source_id.clone(), vec![input]));
        }
    }
    groups
}

fn grouped(inputs: &[&Input], group: &str, zone: &str) -> Result<Vec<Value>, String> {
    let mut groups: Vec<(String, Vec<f64>)> = Vec::new();
    for input in inputs {
        let date = calendar_date(&input.recorded_at, zone)?;
        let key = match group {
            "day" => date,
            "week" => start_of_week(&date),
            "month" => date.chars().take(7).collect(),
            _ => "all".into(),
        };
        if let Some(existing) = groups.iter_mut().find(|item| item.0 == key) {
            existing.1.push(input.value);
        } else {
            groups.push((key, vec![input.value]));
        }
    }
    groups.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(groups
        .into_iter()
        .map(|(period, values)| {
            let mut object = json!({ "period": period });
            merge(&mut object, &summarize(&values));
            object
        })
        .collect())
}

fn daily_means(inputs: &[Input], zone: &str) -> Result<std::collections::BTreeMap<String, f64>, String> {
    let mut days: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
    for input in inputs {
        days.entry(calendar_date(&input.recorded_at, zone)?).or_default().push(input.value);
    }
    Ok(days
        .into_iter()
        .map(|(date, values)| (date, values.iter().sum::<f64>() / values.len() as f64))
        .collect())
}

fn pearson(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;
    let mut numerator = 0.0;
    let mut left_squares = 0.0;
    let mut right_squares = 0.0;
    for index in 0..left.len() {
        let left_difference = left[index] - left_mean;
        let right_difference = right[index] - right_mean;
        numerator += left_difference * right_difference;
        left_squares += left_difference.powi(2);
        right_squares += right_difference.powi(2);
    }
    let denominator = (left_squares * right_squares).sqrt();
    if denominator == 0.0 { None } else { Some(round4(numerator / denominator)) }
}

fn receipt(
    options: &Value,
    unit: &str,
    inputs: &[Input],
    range: Value,
    method: &str,
    result: Value,
    exclusions: &[&str],
) -> Value {
    let mut sources = inputs.iter().map(|input| input.source_id.clone()).collect::<Vec<_>>();
    sources.sort();
    sources.dedup();
    let references: Vec<Value> = inputs
        .iter()
        .take(MAX_REFS)
        .map(|input| json!({ "record_id": input.record_id, "revision": input.revision }))
        .collect();
    let mut body = json!({
        "calculation_version": "ath-runtime-v1",
        "operation": options.get("operation").cloned().unwrap_or(Value::Null),
        "record_kind": options.get("record_kind").and_then(Value::as_str).unwrap_or("measurement"),
        "method": method,
        "quantity": options.get("quantity").cloned().unwrap_or(Value::Null),
        "unit": unit,
        "range": range,
        "sources": sources,
        "input": {
            "record_count": inputs.len(),
            "record_references": references,
            "references_truncated": inputs.len() > MAX_REFS,
        },
        "exclusions": exclusions,
        "result": result,
    });
    if let Some(period) = options.get("provider_period_type").filter(|value| !value.is_null()) {
        body["provider_period_type"] = period.clone();
    }
    body
}

fn one_unit(inputs: &[Input], expected: Option<&str>) -> Result<String, String> {
    let mut units = inputs.iter().map(|input| input.unit.clone()).collect::<Vec<_>>();
    units.sort();
    units.dedup();
    if units.len() != 1 {
        return Err(format!("Calculation matched incompatible units: {}", units.join(", ")));
    }
    if let Some(expected) = expected {
        if units[0] != expected {
            return Err(format!("Calculation expected {expected} but matched {}", units[0]));
        }
    }
    Ok(units.remove(0))
}

fn calendar_date(timestamp: &str, zone: &str) -> Result<String, String> {
    let tz: Tz = zone.parse().map_err(|_| format!("'{zone}' is not an IANA timezone such as America/Chicago"))?;
    let instant = DateTime::parse_from_rfc3339(timestamp).map_err(|error| error.to_string())?;
    let local = instant.with_timezone(&tz);
    Ok(local.format("%Y-%m-%d").to_string())
}

fn calendar_dates(from: &str, to: &str) -> Vec<String> {
    let mut dates = Vec::new();
    let mut current = Utc.with_ymd_and_hms(
        from[0..4].parse().unwrap_or(1970),
        from[5..7].parse().unwrap_or(1),
        from[8..10].parse().unwrap_or(1),
        0, 0, 0,
    ).single().unwrap_or_else(Utc::now);
    let end = Utc.with_ymd_and_hms(
        to[0..4].parse().unwrap_or(1970),
        to[5..7].parse().unwrap_or(1),
        to[8..10].parse().unwrap_or(1),
        0, 0, 0,
    ).single().unwrap_or(current);
    while current <= end {
        dates.push(current.format("%Y-%m-%d").to_string());
        current += chrono::Duration::days(1);
    }
    dates
}

fn start_of_week(date: &str) -> String {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_default();
    let day = parsed.weekday().num_days_from_monday();
    (parsed - chrono::Duration::days(day as i64)).format("%Y-%m-%d").to_string()
}

fn period_sort(period: &Value) -> String {
    period.get("provider_date").and_then(Value::as_str)
        .or_else(|| period.get("start_at").and_then(Value::as_str))
        .or_else(|| period.get("provider_period_id").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn merge(target: &mut Value, extra: &Value) {
    if let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn json_range(options: &Value) -> Value {
    let mut range = serde_json::Map::new();
    if let Some(from) = options.get("from").filter(|value| !value.is_null()) {
        range.insert("from".into(), from.clone());
    }
    if let Some(to) = options.get("to").filter(|value| !value.is_null()) {
        range.insert("to".into(), to.clone());
    }
    if let Some(zone) = options.get("time_zone").filter(|value| !value.is_null()) {
        range.insert("time_zone".into(), zone.clone());
    }
    Value::Object(range)
}

fn method_or<'a>(method: &'a str, fallback: &'a str) -> &'a str {
    if method.is_empty() { fallback } else { method }
}

fn req(value: &Value, key: &str) -> Result<String, String> {
    value.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("missing {key}"))
}

fn parse_ms(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value).map(|time| time.timestamp_millis()).unwrap_or(0)
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[allow(dead_code)]
fn _params_used() {
    let _ = params![];
}
