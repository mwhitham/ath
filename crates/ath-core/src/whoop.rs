//! Read a WHOOP CSV export into the same signals the record import expects.

use std::collections::BTreeMap;

use chrono::DateTime;
use serde_json::{json, Map, Value};

pub fn import_whoop(files: &Map<String, Value>, source_id: &str) -> Value {
    let mut hard = Vec::new();
    let mut soft = Vec::new();
    let mut skipped: BTreeMap<String, i64> = BTreeMap::new();
    let mut examples: BTreeMap<String, String> = BTreeMap::new();
    let known = [
        "physiological_cycles.csv",
        "sleeps.csv",
        "workouts.csv",
        "journal_entries.csv",
    ];
    if let Some(text) = file_text(files, "physiological_cycles.csv") {
        import_cycles(&parse_csv(&text), source_id, &mut hard, &mut skipped, &mut examples);
    }
    if let Some(text) = file_text(files, "sleeps.csv") {
        import_sleeps(&parse_csv(&text), source_id, &mut hard, &mut skipped, &mut examples);
    }
    if let Some(text) = file_text(files, "workouts.csv") {
        import_workouts(&parse_csv(&text), source_id, &mut hard, &mut skipped, &mut examples);
    }
    if let Some(text) = file_text(files, "journal_entries.csv") {
        import_journal(&parse_csv(&text), &mut soft, &mut skipped, &mut examples);
    }
    for name in files.keys() {
        if !known.contains(&name.as_str()) {
            bump(&mut skipped, &mut examples, &format!("unrecognized WHOOP file: {name}"), name);
        }
    }
    json!({
        "vendor": "whoop",
        "detail": "WHOOP via CSV export",
        "hard": hard,
        "soft": soft,
        "skipped": skipped,
        "skip_examples": examples,
    })
}

fn import_cycles(
    rows: &[BTreeMap<String, String>],
    source_id: &str,
    hard: &mut Vec<Value>,
    skipped: &mut BTreeMap<String, i64>,
    examples: &mut BTreeMap<String, String>,
) {
    for row in rows {
        let offset = row_offset(row);
        let periods = cycle_period(row);
        let wake_text = cell(row, &["Wake onset", "Cycle end time", "Cycle start time"]);
        let wake = with_offset(wake_text.as_deref(), offset.as_deref());
        let Some(wake) = wake else {
            bump(skipped, examples, "cycle rows with an unreadable timestamp", &describe_time(wake_text.as_deref(), offset.as_deref()));
            continue;
        };
        for (columns, kind, unit) in [
            (vec!["Heart rate variability (ms)", "hrv_ms"], "hrv_rmssd", "ms"),
            (vec!["Resting heart rate (bpm)"], "resting_heart_rate", "bpm"),
            (vec!["Respiratory rate (rpm)"], "respiratory_rate", "brpm"),
            (vec!["Skin temp (celsius)", "skin_temp_celsius"], "skin_temperature", "°C"),
            (vec!["Blood oxygen %"], "oxygen_saturation", "%"),
        ] {
            let Some(value) = positive(row, &columns) else { continue };
            hard.push(json!({
                "signal": { "type": kind, "value": round2(value), "unit": unit, "recorded_at": wake, "source": source_id },
                "receipt": receipt(wake_text.as_deref().unwrap_or(""), offset.as_deref(), Some(value), Some(unit), None, periods.clone()),
            }));
        }
        if let Some(recovery) = num(row, &["Recovery score %", "recovery_score"]) {
            hard.push(json!({
                "signal": { "type": "vendor_score", "metric": "recovery", "value": recovery, "scale": "0-100", "recorded_at": wake, "source": source_id },
                "receipt": receipt(wake_text.as_deref().unwrap_or(""), offset.as_deref(), Some(recovery), Some("%"), None, periods.clone()),
            }));
        }
        if let Some(strain) = num(row, &["Day Strain", "day_strain"]) {
            hard.push(json!({
                "signal": { "type": "vendor_score", "metric": "strain", "value": round2(strain), "scale": "0-21", "recorded_at": wake, "source": source_id },
                "receipt": receipt_missing_unit(wake_text.as_deref().unwrap_or(""), offset.as_deref(), strain, periods.clone()),
            }));
        }
    }
}

fn import_sleeps(
    rows: &[BTreeMap<String, String>],
    source_id: &str,
    hard: &mut Vec<Value>,
    skipped: &mut BTreeMap<String, i64>,
    examples: &mut BTreeMap<String, String>,
) {
    for row in rows {
        let offset = row_offset(row);
        let periods = cycle_period(row);
        let start_text = cell(row, &["Sleep onset"]);
        let end_text = cell(row, &["Wake onset"]);
        let start = with_offset(start_text.as_deref(), offset.as_deref());
        let end = with_offset(end_text.as_deref(), offset.as_deref());
        if start.is_none() || end.is_none() || parse_ms(end.as_deref().unwrap_or("")) <= parse_ms(start.as_deref().unwrap_or("")) {
            bump(skipped, examples, "sleep rows with an unusable time range", &format!("{} → {}", describe_time(start_text.as_deref(), offset.as_deref()), describe_time(end_text.as_deref(), offset.as_deref())));
            continue;
        }
        let start = start.unwrap();
        let end = end.unwrap();
        let mut aggregates = Map::new();
        let mut summaries = Map::new();
        let set = |key: &str, minutes: Option<f64>, aggregates: &mut Map<String, Value>, summaries: &mut Map<String, Value>| {
            if let Some(minutes) = minutes {
                aggregates.insert(key.into(), json!((minutes * 60.0).round()));
                summaries.insert(key.into(), json!({ "input_value": minutes, "input_unit": "min" }));
            }
        };
        set("duration_s", positive(row, &["Asleep duration (min)"]), &mut aggregates, &mut summaries);
        set("time_in_bed_s", positive(row, &["In bed duration (min)"]), &mut aggregates, &mut summaries);
        set("light_s", positive(row, &["Light sleep duration (min)"]), &mut aggregates, &mut summaries);
        set("deep_s", positive(row, &["Deep (SWS) duration (min)", "deep_sleep_duration_min"]), &mut aggregates, &mut summaries);
        set("rem_s", positive(row, &["REM duration (min)"]), &mut aggregates, &mut summaries);
        set("awake_s", positive(row, &["Awake duration (min)"]), &mut aggregates, &mut summaries);
        if let Some(efficiency) = positive(row, &["Sleep efficiency %"]) {
            let shown = ((efficiency * 10.0).round() / 10.0).min(100.0);
            aggregates.insert("efficiency_pct".into(), json!(shown));
            summaries.insert("efficiency_pct".into(), json!({ "input_value": efficiency, "input_unit": "%" }));
        }
        hard.push(json!({
            "signal": { "type": "sleep_session", "start": start, "end": end, "source": source_id, "aggregates": aggregates },
            "receipt": {
                "input_start_timestamp": timestamp_receipt(start_text.as_deref().unwrap_or(""), offset.as_deref()),
                "input_end_timestamp": timestamp_receipt(end_text.as_deref().unwrap_or(""), offset.as_deref()),
                "input_summaries": summaries,
            }
        }));
        if let Some(last) = hard.last_mut() {
            if let Some(periods) = periods.clone() {
                last["receipt"]["provider_reporting_periods"] = json!([periods]);
            }
        }
    }
}

fn import_workouts(
    rows: &[BTreeMap<String, String>],
    source_id: &str,
    hard: &mut Vec<Value>,
    skipped: &mut BTreeMap<String, i64>,
    examples: &mut BTreeMap<String, String>,
) {
    for row in rows {
        let offset = row_offset(row);
        let periods = cycle_period(row);
        let start_text = cell(row, &["Workout start time"]);
        let end_text = cell(row, &["Workout end time"]);
        let start = with_offset(start_text.as_deref(), offset.as_deref());
        let end = with_offset(end_text.as_deref(), offset.as_deref());
        if start.is_none() || end.is_none() || parse_ms(end.as_deref().unwrap_or("")) <= parse_ms(start.as_deref().unwrap_or("")) {
            bump(skipped, examples, "workout rows with an unusable time range", &format!("{} → {}", describe_time(start_text.as_deref(), offset.as_deref()), describe_time(end_text.as_deref(), offset.as_deref())));
            continue;
        }
        let start = start.unwrap();
        let end = end.unwrap();
        let mut aggregates = Map::new();
        if let Some(activity) = cell(row, &["Activity name"]) {
            aggregates.insert("activity".into(), json!(activity.to_lowercase()));
        }
        let avg = positive(row, &["Average HR (bpm)", "average_hr_bpm"]);
        let max = positive(row, &["Max HR (bpm)", "max_hr_bpm"]);
        let energy = positive(row, &["Energy burned (cal)"]);
        let distance = positive(row, &["Distance (meters)", "distance_meters"]);
        if let Some(avg) = avg { aggregates.insert("avg_hr_bpm".into(), json!(avg.round())); }
        if let Some(max) = max { aggregates.insert("max_hr_bpm".into(), json!(max.round())); }
        if let Some(energy) = energy { aggregates.insert("energy_kcal".into(), json!(energy.round())); }
        if let Some(distance) = distance { aggregates.insert("distance_m".into(), json!(distance.round())); }
        let mut summaries = Map::new();
        if let Some(avg) = avg { summaries.insert("avg_hr_bpm".into(), json!({ "input_value": avg, "input_unit": "bpm" })); }
        if let Some(max) = max { summaries.insert("max_hr_bpm".into(), json!({ "input_value": max, "input_unit": "bpm" })); }
        if let Some(energy) = energy { summaries.insert("energy_kcal".into(), json!({ "input_value": energy, "input_unit": "cal" })); }
        if let Some(distance) = distance { summaries.insert("distance_m".into(), json!({ "input_value": distance, "input_unit": "m" })); }
        hard.push(json!({
            "signal": { "type": "workout_session", "start": start, "end": end, "source": source_id, "aggregates": aggregates },
            "receipt": {
                "input_start_timestamp": timestamp_receipt(start_text.as_deref().unwrap_or(""), offset.as_deref()),
                "input_end_timestamp": timestamp_receipt(end_text.as_deref().unwrap_or(""), offset.as_deref()),
                "input_summaries": summaries,
            }
        }));
        if let Some(last) = hard.last_mut() {
            if let Some(periods) = periods.clone() {
                last["receipt"]["provider_reporting_periods"] = json!([periods]);
            }
        }
        if let Some(strain) = num(row, &["Activity Strain", "activity_strain"]) {
            hard.push(json!({
                "signal": { "type": "vendor_score", "metric": "activity_strain", "value": round2(strain), "scale": "0-21", "recorded_at": start, "source": source_id },
                "receipt": receipt_missing_unit(&start_text.clone().unwrap_or_default(), offset.as_deref(), strain, periods.clone()),
            }));
        }
    }
}

fn import_journal(
    rows: &[BTreeMap<String, String>],
    soft: &mut Vec<Value>,
    skipped: &mut BTreeMap<String, i64>,
    examples: &mut BTreeMap<String, String>,
) {
    for row in rows {
        let offset = row_offset(row);
        let reported_text = cell(row, &["Cycle end time", "Cycle start time"]);
        let reported = with_offset(reported_text.as_deref(), offset.as_deref());
        let question = cell(row, &["Question text", "question_text"]);
        if reported.is_none() || question.is_none() {
            let example = if question.is_none() {
                "row has no question text".to_string()
            } else if cell(row, &["Cycle end time", "Cycle start time"]).is_none() {
                "row has neither a cycle start nor end time".to_string()
            } else {
                describe_time(reported_text.as_deref(), offset.as_deref())
            };
            bump(skipped, examples, "journal rows we cannot place in time", &example);
            continue;
        }
        let answer = cell(row, &["Answered yes", "answered_yes", "Answer"]).unwrap_or_default();
        let notes = cell(row, &["Notes"]);
        let question = question.unwrap_or_default();
        let affirmative = matches!(answer.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1");
        let mut note = format!("{question}: {}", if affirmative { "yes" } else { "no" });
        if let Some(notes) = notes {
            note = format!("{note} — {notes}");
        }
        let kind = journal_type(&question);
        soft.push(json!({
            "signal": { "type": kind, "reported_at": reported, "note": note, "provenance": { "via": "text" } },
            "receipt": { "input_timestamp": timestamp_receipt(reported_text.as_deref().unwrap_or(""), offset.as_deref()) }
        }));
    }
}

fn journal_type(question: &str) -> &'static str {
    let lower = question.to_lowercase();
    if ["alcohol", "caffeine", "coffee", "ate ", "eat ", "meal", "hydrat", "water", "supplement", "creatine"].iter().any(|word| lower.contains(word)) { "nutrition" }
    else if ["stress", "anxious", "anxiety", "meditat", "breathwork", "journal"].iter().any(|word| lower.contains(word)) { "stress" }
    else if ["sleep", "bed", "nap", "blue light", "screen"].iter().any(|word| lower.contains(word)) { "sleep_quality" }
    else if ["sore", "injur", "pain"].iter().any(|word| lower.contains(word)) { "soreness" }
    else if ["energy", "fatigue", "tired"].iter().any(|word| lower.contains(word)) { "energy" }
    else if ["mood", "happy", "sad"].iter().any(|word| lower.contains(word)) { "mood" }
    else { "note" }
}

fn cycle_period(row: &BTreeMap<String, String>) -> Option<Value> {
    let offset = row_offset(row);
    let input_start = cell(row, &["Cycle start time"]);
    let input_end = cell(row, &["Cycle end time"]);
    let start = with_offset(input_start.as_deref(), offset.as_deref())?;
    let end = with_offset(input_end.as_deref(), offset.as_deref());
    let mut period = json!({
        "period_type": "whoop_physiological_cycle",
        "start_at": to_utc(&start),
        "input_start_timestamp": timestamp_receipt(input_start.as_deref().unwrap_or(""), offset.as_deref()),
    });
    if let Some(end) = end {
        if input_end.is_some() {
            period["end_at"] = json!(to_utc(&end));
            period["input_end_timestamp"] = json!(timestamp_receipt(input_end.as_deref().unwrap_or(""), offset.as_deref()));
        }
    }
    if let Some(offset) = offset {
        period["timezone_offset_text"] = json!(offset);
        period["timezone_offset_meaning"] = json!("provider_cycle_offset");
    } else {
        period["timezone_offset_missing_reason"] = json!("not_provided_by_source");
    }
    Some(period)
}

fn receipt(text: &str, offset: Option<&str>, value: Option<f64>, unit: Option<&str>, missing: Option<&str>, periods: Option<Value>) -> Value {
    let mut body = json!({ "input_timestamp": timestamp_receipt(text, offset) });
    if let Some(value) = value { body["input_value"] = json!(value); }
    if let Some(unit) = unit { body["input_unit"] = json!(unit); }
    if let Some(missing) = missing { body["input_unit_missing_reason"] = json!(missing); }
    if let Some(periods) = periods { body["provider_reporting_periods"] = json!([periods]); }
    body
}

fn receipt_missing_unit(text: &str, offset: Option<&str>, value: f64, periods: Option<Value>) -> Value {
    receipt(text, offset, Some(value), None, Some("not_provided_by_source"), periods)
}

fn timestamp_receipt(text: &str, offset: Option<&str>) -> Value {
    let mut body = json!({
        "timestamp_text": text,
        "timestamp_basis": "source_local_timestamp_with_documented_offset",
    });
    if let Some(offset) = offset {
        body["utc_offset_text"] = json!(offset);
        body["utc_offset_meaning"] = json!("provider_cycle_offset");
    } else {
        body["utc_offset_missing_reason"] = json!("not_provided_by_source");
    }
    body
}

fn file_text(files: &Map<String, Value>, name: &str) -> Option<String> {
    files.get(name).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn parse_csv(text: &str) -> Vec<BTreeMap<String, String>> {
    let mut lines = text.trim().lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let delimiter = if header.matches(';').count() > header.matches(',').count() { ';' } else { ',' };
    let headers = split_csv(header, delimiter).into_iter().map(|name| normalize(&name)).collect::<Vec<_>>();
    lines.filter_map(|line| {
        if line.trim().is_empty() { return None; }
        let cells = split_csv(line, delimiter);
        let mut row = BTreeMap::new();
        for (index, header) in headers.iter().enumerate() {
            if let Some(value) = cells.get(index) {
                row.insert(header.clone(), value.clone());
            }
        }
        if row.is_empty() { None } else { Some(row) }
    }).collect()
}

fn split_csv(line: &str, delimiter: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if quoted && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else if quoted {
                quoted = false;
            } else if current.is_empty() {
                quoted = true;
            } else {
                current.push('"');
            }
        } else if ch == delimiter && !quoted {
            out.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    out.push(current.trim().to_string());
    out
}

fn normalize(header: &str) -> String {
    let lower = header.to_lowercase().replace(['(', ')', '%'], " ");
    let mut out = String::new();
    let mut underscore = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            underscore = false;
        } else if !underscore && !out.is_empty() {
            out.push('_');
            underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

pub(crate) fn cell(row: &BTreeMap<String, String>, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = row.get(&normalize(name)) {
            if !value.trim().is_empty() { return Some(value.trim().to_string()); }
        }
    }
    None
}

pub(crate) fn num(row: &BTreeMap<String, String>, names: &[&str]) -> Option<f64> {
    let raw = cell(row, names)?;
    raw.replace(',', "").parse::<f64>().ok().filter(|value| value.is_finite())
}

pub(crate) fn positive(row: &BTreeMap<String, String>, names: &[&str]) -> Option<f64> {
    num(row, names).filter(|value| *value > 0.0)
}

fn row_offset(row: &BTreeMap<String, String>) -> Option<String> {
    cell(row, &["Cycle timezone", "cycle_timezone", "timezone_offset"])
}

pub(crate) fn with_offset(local: Option<&str>, offset: Option<&str>) -> Option<String> {
    let trimmed = local?.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 16 { return None; }
    let y = &trimmed[0..4];
    let mo = &trimmed[5..7];
    let d = &trimmed[8..10];
    let h = &trimmed[11..13];
    let mi = &trimmed[14..16];
    if !y.chars().all(|ch| ch.is_ascii_digit()) { return None; }
    let s = if trimmed.len() >= 19 && trimmed.as_bytes().get(16) == Some(&b':') { &trimmed[17..19] } else { "00" };
    if offset_at_end(trimmed).is_some() {
        return Some(trimmed.replace(' ', "T"));
    }
    let normalized = normalize_offset(offset)?;
    Some(format!("{y}-{mo}-{d}T{h}:{mi}:{s}{normalized}"))
}

fn offset_at_end(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.ends_with('Z') || trimmed.ends_with('z') {
        return Some("Z".into());
    }
    let bytes = trimmed.as_bytes();
    let mut index = trimmed.len();
    while index > 0 {
        index -= 1;
        let ch = bytes[index] as char;
        if ch == 'T' {
            break;
        }
        if ch == '+' || ch == '-' {
            let rest = &trimmed[index..];
            if rest.len() >= 6 && rest.as_bytes().get(3) == Some(&b':') {
                return Some(rest.to_string());
            }
            if rest.len() >= 5 && rest[1..].chars().all(|item| item.is_ascii_digit()) {
                return Some(format!("{}{}:{}", &rest[0..1], &rest[1..3], &rest[3..5]));
            }
        }
    }
    None
}

fn normalize_offset(offset: Option<&str>) -> Option<String> {
    let trimmed = offset?.trim();
    if trimmed == "Z" || trimmed.eq_ignore_ascii_case("UTC") || trimmed.eq_ignore_ascii_case("GMT") { return Some("Z".into()); }
    let bare = trimmed.trim_start_matches(|ch: char| ch.is_ascii_alphabetic()).trim();
    let bare = if trimmed.to_ascii_uppercase().starts_with("UTC") || trimmed.to_ascii_uppercase().starts_with("GMT") {
        trimmed.get(3..).unwrap_or("").trim()
    } else { bare };
    if bare.is_empty() || bare.eq_ignore_ascii_case("Z") || bare == "+00:00" || bare == "-00:00" || bare == "+0000" || bare == "-0000" {
        return Some("Z".into());
    }
    let sign = bare.chars().next()?;
    if sign != '+' && sign != '-' { return None; }
    let digits = &bare[1..].replace(':', "");
    if digits.len() < 3 { return None; }
    let hours = if digits.len() == 3 { format!("0{}", &digits[0..1]) } else { digits[0..2].to_string() };
    let minutes = if digits.len() == 3 { digits[1..3].to_string() } else { digits[2..4].to_string() };
    Some(format!("{sign}{hours}:{minutes}"))
}

fn to_utc(value: &str) -> String {
    DateTime::parse_from_rfc3339(value).map(|time| time.to_utc().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string().replace(".000Z", ".000Z")).unwrap_or_else(|_| value.to_string())
}

fn parse_ms(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value).map(|time| time.timestamp_millis()).unwrap_or(0)
}

fn describe_time(value: Option<&str>, offset: Option<&str>) -> String {
    match value {
        None => "blank timestamp".into(),
        Some(value) => format!("\"{value}\" with timezone \"{}\"", offset.unwrap_or("(none)")),
    }
}

fn bump(skipped: &mut BTreeMap<String, i64>, examples: &mut BTreeMap<String, String>, reason: &str, example: &str) {
    *skipped.entry(reason.into()).or_insert(0) += 1;
    examples.entry(reason.into()).or_insert_with(|| example.to_string());
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{import_whoop, with_offset};
    use serde_json::json;

    #[test]
    fn fractional_second_offset_is_kept() {
        let parsed = with_offset(Some("2026-08-09T18:00:00.000-04:00"), None).unwrap();
        assert_eq!(parsed, "2026-08-09T18:00:00.000-04:00");
    }

    #[test]
    fn whoop_cycle_keeps_hrv_and_a_timestamp() {
        let csv = "Cycle start time,Cycle end time,Cycle timezone,Heart rate variability (ms),Resting heart rate (bpm)\n2026-08-09 06:00:00,2026-08-09 14:00:00,UTC-04:00,42,51\n";
        let mut files = serde_json::Map::new();
        files.insert("physiological_cycles.csv".into(), json!(csv));
        let result = import_whoop(&files, "whoop-1");
        let hard = result.get("hard").and_then(|value| value.as_array()).unwrap();
        assert!(hard.iter().any(|item| item["signal"]["type"] == "hrv_rmssd" && item["signal"]["recorded_at"].as_str().unwrap().contains("-04:00")));
    }
}
