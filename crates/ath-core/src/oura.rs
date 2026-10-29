//! Read an Oura CSV export into the same signals the record import expects.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::whoop::{cell, num, parse_csv, positive, with_offset};

const SLEEP_POINTS: &[(&[&str], &str, &str)] = &[
    (&["average_hrv", "Average HRV", "hrv"], "hrv_rmssd", "ms"),
    (&["lowest_heart_rate", "Lowest Resting Heart Rate", "lowest_resting_heart_rate"], "resting_heart_rate", "bpm"),
    (&["average_breath", "Respiratory Rate", "respiratory_rate"], "respiratory_rate", "brpm"),
    (&["spo2_percentage", "Average SpO2", "spo2"], "oxygen_saturation", "%"),
    (&["vo2_max"], "vo2_max", "ml/kg/min"),
];

const SLEEP_STAGES: &[(&[&str], &str)] = &[
    (&["total_sleep_duration", "Total Sleep Duration"], "duration_s"),
    (&["time_in_bed", "Time in Bed"], "time_in_bed_s"),
    (&["deep_sleep_duration", "Deep Sleep Duration"], "deep_s"),
    (&["rem_sleep_duration", "REM Sleep Duration"], "rem_s"),
    (&["light_sleep_duration", "Light Sleep Duration"], "light_s"),
    (&["awake_time", "Awake Time", "awake_duration"], "awake_s"),
];

const VENDOR_SCORES: &[(&[&str], &str)] = &[
    (&["readiness_score", "Readiness Score"], "readiness"),
    (&["sleep_score", "Sleep Score"], "sleep_score"),
    (&["activity_score", "Activity Score"], "activity_score"),
];

const CONTRIBUTORS: &[&str] = &[
    "activity_balance", "body_temperature", "hrv_balance", "previous_day_activity", "previous_night",
    "recovery_index", "resting_heart_rate", "sleep_balance", "sleep_regularity",
];

const ACTIVITY_COLUMNS: &[&str] = &[
    "steps", "active_calories", "total_calories", "target_calories", "met_minutes",
    "low_activity_met_minutes", "medium_activity_met_minutes", "high_activity_met_minutes",
    "equivalent_walking_distance", "target_meters", "meters_to_target", "inactivity_alerts",
    "resting_time", "sedentary_met_minutes", "low_activity_time", "medium_activity_time",
    "high_activity_time", "sedentary_time", "non_wear_time", "average_met_minutes", "met", "class_5_min",
];

pub fn import_oura(files: &Map<String, Value>, source_id: &str) -> Value {
    let mut hard = Vec::new();
    let mut skipped: BTreeMap<String, i64> = BTreeMap::new();
    for (name, text) in files {
        let Some(text) = text.as_str() else { continue };
        let rows = parse_csv(text);
        if rows.is_empty() {
            *skipped.entry(format!("empty CSV: {name}")).or_insert(0) += 1;
            continue;
        }
        import_rows(&rows, source_id, name, &mut hard, &mut skipped);
    }
    json!({
        "vendor": "oura",
        "detail": "Oura via CSV export",
        "hard": hard,
        "soft": [],
        "skipped": skipped,
        "skip_examples": {},
    })
}

fn import_rows(
    rows: &[BTreeMap<String, String>],
    source_id: &str,
    name: &str,
    hard: &mut Vec<Value>,
    skipped: &mut BTreeMap<String, i64>,
) {
    let mut used = 0;
    let portal = detect_portal(rows);
    for row in rows {
        let bedtime_start_text = cell(row, &["bedtime_start", "Bedtime Start"]);
        let bedtime_end_text = cell(row, &["bedtime_end", "Bedtime End"]);
        let bedtime_start = with_offset(bedtime_start_text.as_deref(), None);
        let bedtime_end = with_offset(bedtime_end_text.as_deref(), None);
        let day = cell(row, &["day", "date", "summary_date"]);
        let sleep_periods = period(day.as_deref(), "oura_sleep_day");
        let activity_periods = period(day.as_deref(), "oura_activity_day");
        let unspecified_periods = period(day.as_deref(), "oura_day_unspecified");
        let anchor = bedtime_end.clone().or_else(|| day.as_ref().map(|day| format!("{day}T00:00:00Z")));
        let Some(anchor) = anchor else {
            *skipped.entry(format!("{name}: rows without a date")).or_insert(0) += 1;
            continue;
        };
        used += 1;
        if let Some(spo2) = cell(row, &["spo2_percentage", "Average SpO2", "spo2"]) {
            if spo2.starts_with('{') {
                if let Some(value) = json_number(&spo2) {
                    if value > 0.0 {
                        hard.push(point("oxygen_saturation", round2(value), "%", &anchor, source_id, None, Some(value), Some("%"), unspecified_periods.clone()));
                    }
                }
            }
        }
        for (columns, kind, unit) in SLEEP_POINTS {
            if *kind == "oxygen_saturation" {
                if let Some(raw) = cell(row, columns) {
                    if raw.trim().starts_with('{') { continue; }
                }
            }
            let Some(value) = positive(row, columns) else { continue };
            hard.push(point(kind, round2(value), unit, &anchor, source_id, bedtime_end_text.as_deref(), Some(value), Some(unit), sleep_periods.clone()));
        }
        if let Some(deviation) = num(row, &["temperature_deviation", "Temperature Deviation"]) {
            hard.push(point("temperature_deviation", round2(deviation), "°C", &anchor, source_id, bedtime_end_text.as_deref(), Some(deviation), Some("°C"), unspecified_periods.clone()));
        }
        if let (Some(start), Some(end)) = (bedtime_start, bedtime_end) {
            if parse_ms(&end) > parse_ms(&start) {
                let mut aggregates = Map::new();
                let mut summaries = Map::new();
                for (columns, key) in SLEEP_STAGES {
                    if let Some(seconds) = positive(row, columns) {
                        aggregates.insert((*key).into(), json!(seconds.round()));
                        summaries.insert((*key).into(), json!({ "input_value": seconds, "input_unit": "s" }));
                    }
                }
                if let Some(efficiency) = positive(row, &["efficiency", "sleep_efficiency", "Sleep Efficiency"]) {
                    aggregates.insert("efficiency_pct".into(), json!(((efficiency * 10.0).round() / 10.0).min(100.0)));
                    summaries.insert("efficiency_pct".into(), json!({ "input_value": efficiency, "input_unit": "%" }));
                }
                let mut receipt = json!({
                    "input_start_timestamp": oura_receipt(bedtime_start_text.as_deref().unwrap_or("")),
                    "input_end_timestamp": oura_receipt(bedtime_end_text.as_deref().unwrap_or("")),
                    "input_summaries": summaries,
                });
                if let Some(periods) = sleep_periods.clone() {
                    receipt["provider_reporting_periods"] = json!([periods]);
                }
                hard.push(json!({
                    "signal": { "type": "sleep_session", "start": start, "end": end, "source": source_id, "aggregates": aggregates },
                    "receipt": receipt,
                }));
            }
        }
        let score = num(row, &["score", "Score"]);
        let contributors = cell(row, &["contributors"]);
        let keys = contributor_keys(contributors.as_deref());
        if score.is_some() && contributors.is_some() && keys.as_ref().is_some_and(|keys| keys.is_empty()) {
            *skipped.entry(format!("{name}: score row with empty contributors")).or_insert(0) += 1;
        }
        if let (Some(score), Some(metric), Some(keys)) = (score, portal, keys.as_ref()) {
            if contributors.is_some() && !keys.is_empty() {
                let periods = match metric {
                    "sleep_score" => sleep_periods.clone(),
                    "activity_score" => activity_periods.clone(),
                    _ => unspecified_periods.clone(),
                };
                hard.push(score_signal(metric, score, &anchor, source_id, None, periods));
            }
        }
        if num(row, &["breathing_disturbance_index", "breathing_disturbance"]).is_some() {
            *skipped.entry(format!("{name}: breathing disturbance column")).or_insert(0) += 1;
        }
        if num(row, &["cardiovascular_age"]).is_some() {
            *skipped.entry(format!("{name}: cardiovascular age column")).or_insert(0) += 1;
        }
        for column in ACTIVITY_COLUMNS {
            let present = if *column == "met" { cell(row, &[column]).is_some() } else { num(row, &[column]).is_some() };
            if present {
                *skipped.entry(format!("{name}: activity column {column}")).or_insert(0) += 1;
            }
        }
        for (columns, metric) in VENDOR_SCORES {
            let Some(value) = num(row, columns) else { continue };
            let periods = match *metric {
                "activity_score" => activity_periods.clone(),
                "sleep_score" => sleep_periods.clone(),
                _ => unspecified_periods.clone(),
            };
            hard.push(score_signal(metric, value, &anchor, source_id, bedtime_end_text.as_deref(), periods));
        }
        let mut contributor_values: BTreeMap<String, f64> = BTreeMap::new();
        if let Some(text) = contributors.as_deref() {
            if text.starts_with('{') {
                match serde_json::from_str::<Map<String, Value>>(text) {
                    Ok(parsed) => {
                        for (key, value) in parsed {
                            if let Some(number) = value.as_f64() {
                                if CONTRIBUTORS.contains(&key.as_str()) {
                                    contributor_values.insert(key, number);
                                } else {
                                    *skipped.entry(format!("{name}: contributor column {key}")).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        *skipped.entry(format!("{name}: contributors cell is not valid JSON")).or_insert(0) += 1;
                    }
                }
            }
        }
        for contributor in CONTRIBUTORS {
            let value = contributor_values.get(*contributor).copied().or_else(|| num(row, &[&format!("contributor_{contributor}"), &format!("contributors_{contributor}")]));
            let Some(value) = value else { continue };
            hard.push(score_signal(&format!("readiness_contributor_{contributor}"), value, &anchor, source_id, bedtime_end_text.as_deref(), unspecified_periods.clone()));
        }
    }
    if used == 0 {
        *skipped.entry(format!("{name}: no rows we could place")).or_insert(0) += 1;
    }
}

fn point(
    kind: &str,
    value: f64,
    unit: &str,
    anchor: &str,
    source_id: &str,
    timestamp: Option<&str>,
    input_value: Option<f64>,
    input_unit: Option<&str>,
    periods: Option<Value>,
) -> Value {
    let mut receipt = json!({});
    if let Some(timestamp) = timestamp {
        receipt["input_timestamp"] = json!(oura_receipt(timestamp));
    }
    if let Some(input_value) = input_value { receipt["input_value"] = json!(input_value); }
    if let Some(input_unit) = input_unit { receipt["input_unit"] = json!(input_unit); }
    if let Some(periods) = periods { receipt["provider_reporting_periods"] = json!([periods]); }
    json!({
        "signal": { "type": kind, "value": value, "unit": unit, "recorded_at": anchor, "source": source_id },
        "receipt": receipt,
    })
}

fn score_signal(metric: &str, value: f64, anchor: &str, source_id: &str, timestamp: Option<&str>, periods: Option<Value>) -> Value {
    let mut receipt = json!({
        "input_value": value,
        "input_unit_missing_reason": "not_provided_by_source",
    });
    if let Some(timestamp) = timestamp {
        receipt["input_timestamp"] = json!(oura_receipt(timestamp));
    }
    if let Some(periods) = periods { receipt["provider_reporting_periods"] = json!([periods]); }
    json!({
        "signal": { "type": "vendor_score", "metric": metric, "value": value, "scale": "0-100", "recorded_at": anchor, "source": source_id },
        "receipt": receipt,
    })
}

fn period(day: Option<&str>, period_type: &str) -> Option<Value> {
    let day = day?;
    Some(json!({
        "period_type": period_type,
        "provider_date": day,
        "timezone_offset_missing_reason": "not_provided_by_source",
    }))
}

fn oura_receipt(timestamp: &str) -> Value {
    let mut body = json!({
        "timestamp_text": timestamp,
        "timestamp_basis": "source_offset_timestamp",
    });
    if let Some(offset) = offset_suffix(timestamp) {
        body["utc_offset_text"] = json!(offset);
        body["utc_offset_meaning"] = json!("meaning_not_documented");
    } else {
        body["utc_offset_missing_reason"] = json!("not_provided_by_source");
    }
    body
}

fn offset_suffix(timestamp: &str) -> Option<String> {
    let trimmed = timestamp.trim();
    if trimmed.ends_with('Z') || trimmed.ends_with('z') { return Some(trimmed[trimmed.len()-1..].to_string()); }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 6 {
        let start = bytes.len() - 6;
        let sign = bytes[start] as char;
        if (sign == '+' || sign == '-') && bytes.get(start + 3) == Some(&b':') {
            return Some(trimmed[start..].to_string());
        }
    }
    None
}

fn json_number(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if !trimmed.starts_with('{') {
        return trimmed.replace(',', "").parse::<f64>().ok().filter(|value| value.is_finite());
    }
    let parsed: Map<String, Value> = serde_json::from_str(trimmed).ok()?;
    parsed.get("average").and_then(Value::as_f64).or_else(|| parsed.get("score").and_then(Value::as_f64))
}

fn contributor_keys(text: Option<&str>) -> Option<Vec<String>> {
    let text = text?.trim();
    if text.is_empty() || text == "{}" { return Some(Vec::new()); }
    if !text.starts_with('{') { return None; }
    let parsed: Map<String, Value> = serde_json::from_str(text).ok()?;
    Some(parsed.keys().cloned().collect())
}

fn detect_portal(rows: &[BTreeMap<String, String>]) -> Option<&'static str> {
    for row in rows {
        let Some(keys) = contributor_keys(cell(row, &["contributors"]).as_deref()) else { continue };
        if keys.is_empty() { continue; }
        if let Some(metric) = portal_metric(&keys) { return Some(metric); }
    }
    None
}

fn portal_metric(keys: &[String]) -> Option<&'static str> {
    let readiness_keys = CONTRIBUTORS;
    let sleep_keys = ["timing", "latency", "deep_sleep", "rem_sleep", "total_sleep", "efficiency", "restfulness"];
    let activity_keys = ["stay_active", "move_every_hour", "meet_daily_targets", "training_frequency", "training_volume", "recovery_time"];
    let mut readiness = 0;
    let mut sleep = 0;
    let mut activity = 0;
    for key in keys {
        if readiness_keys.contains(&key.as_str()) { readiness += 1; }
        if sleep_keys.contains(&key.as_str()) { sleep += 1; }
        if activity_keys.contains(&key.as_str()) { activity += 1; }
    }
    let best = readiness.max(sleep).max(activity);
    if best == 0 { return None; }
    if sleep == best && sleep > readiness && sleep >= activity { return Some("sleep_score"); }
    if activity == best && activity > readiness && activity >= sleep { return Some("activity_score"); }
    Some("readiness")
}

fn parse_ms(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value).map(|time| time.timestamp_millis()).unwrap_or(0)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::import_oura;
    use serde_json::json;

    #[test]
    fn fractional_second_bedtimes_become_sleep_and_points() {
        let text = include_str!("../../../tests/fixtures/exports/oura-offset/sleep.csv");
        let mut files = serde_json::Map::new();
        files.insert("sleep.csv".into(), json!(text));
        let result = import_oura(&files, "oura-1");
        let hard = result.get("hard").and_then(|value| value.as_array()).unwrap();
        let kinds: Vec<&str> = hard.iter().filter_map(|item| item["signal"]["type"].as_str()).collect();
        assert!(kinds.contains(&"sleep_session"), "{kinds:?}");
        assert!(kinds.contains(&"hrv_rmssd"), "{kinds:?}");
        assert!(kinds.contains(&"respiratory_rate"), "{kinds:?}");
        assert!(kinds.contains(&"resting_heart_rate"), "{kinds:?}");
        assert!(hard.iter().all(|item| {
            let signal = &item["signal"];
            let time = signal.get("recorded_at").or_else(|| signal.get("start")).and_then(|value| value.as_str()).unwrap_or("");
            time.contains("-04:00")
        }));
    }
}
