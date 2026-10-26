//! Run a calculation skill file against one record.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::store::StoredRecord;

#[derive(Debug, Clone)]
pub struct SkillFile {
    pub name: String,
    pub settings: BTreeMap<String, Vec<String>>,
}

pub fn parse_skill(text: &str) -> Result<SkillFile, String> {
    let start = text
        .find("```ath-calculation")
        .ok_or("the skill file has no ath-calculation block")?;
    let rest = &text[start + "```ath-calculation".len()..];
    let end = rest.find("```").ok_or("the calculation block is not closed")?;
    let mut settings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in rest[..end].lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("could not read '{line}'"))?;
        settings
            .entry(key.trim().to_string())
            .or_default()
            .push(value.trim().to_string());
    }
    let name = settings
        .get("name")
        .and_then(|values| values.first())
        .cloned()
        .ok_or("the calculation block needs a name")?;
    Ok(SkillFile { name, settings })
}

pub fn run_skill(
    skill: &SkillFile,
    record: &StoredRecord,
    related: &[StoredRecord],
) -> Value {
    let mut fields_read = Vec::new();
    let mut missing = Vec::new();
    let result = match skill.name.as_str() {
        "heart-rate-zones" => zones(skill, record, &mut fields_read, &mut missing),
        "heart-rate-recovery" => recovery(skill, record, &mut fields_read, &mut missing),
        "resting-heart-rate" => resting(skill, record, &mut fields_read, &mut missing),
        "training-load-heart-rate" => load_hr(skill, record, &mut fields_read, &mut missing),
        "training-load-effort" => load_effort(skill, record, &mut fields_read, &mut missing),
        "sleep-duration" | "sleep-interruptions" => number_field(skill, record, &mut fields_read, &mut missing),
        "sleep-efficiency" => efficiency(skill, record, &mut fields_read, &mut missing),
        "sleep-timing" => timing(skill, record, &mut fields_read, &mut missing),
        "sleep-stages" => stages(skill, record, &mut fields_read, &mut missing),
        "sleep-regularity" => regularity(skill, record, related, &mut fields_read, &mut missing),
        "strength-volume" => volume(skill, record, &mut fields_read, &mut missing),
        "same-quantity-change" => quantity_change(record, related, &mut fields_read, &mut missing),
        "sleep-change" => sleep_change(skill, record, related, &mut fields_read, &mut missing),
        other => {
            missing.push(format!("unknown calculation {other}"));
            Value::Null
        }
    };
    json!({
        "calculation": skill.name,
        "record_id": record.record_id,
        "fields_read": fields_read,
        "missing": missing,
        "result": result,
    })
}

fn setting<'a>(skill: &'a SkillFile, key: &str) -> Option<&'a str> {
    skill.settings.get(key).and_then(|values| values.first()).map(String::as_str)
}

fn lookup<'a>(data: &'a Value, field: &str, fields_read: &mut Vec<Value>) -> Option<&'a Value> {
    let value = if let Some(name) = field.strip_prefix("supplied:") {
        data.get("supplied_fields").and_then(|fields| fields.get(name))
    } else {
        data.get(field)
    };
    fields_read.push(json!({
        "field": field,
        "present": value.is_some(),
    }));
    value
}

fn samples(value: &Value) -> Vec<(i64, f64)> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let bpm = item.get("bpm")?.as_f64()?;
            let offset = item.get("offset_ms").and_then(Value::as_i64).unwrap_or(0);
            Some((offset, bpm))
        })
        .collect()
}

fn durations(samples: &[(i64, f64)]) -> Vec<(f64, i64)> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for index in 0..samples.len() {
        let duration = if index + 1 < samples.len() {
            samples[index + 1].0 - samples[index].0
        } else if index > 0 {
            samples[index].0 - samples[index - 1].0
        } else {
            0
        };
        out.push((samples[index].1, duration.max(0)));
    }
    out
}

fn zones(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let sample_field = setting(skill, "sample_field").unwrap_or("heart_rate_samples");
    let maximum_field = setting(skill, "maximum_field").unwrap_or("maximum_heart_rate_bpm");
    let sample_value = lookup(&record.data, sample_field, fields_read);
    let maximum = lookup(&record.data, maximum_field, fields_read).and_then(Value::as_f64);
    if sample_value.is_none() {
        missing.push(sample_field.to_string());
    }
    let Some(maximum) = maximum else {
        missing.push(maximum_field.to_string());
        return Value::Null;
    };
    if maximum <= 0.0 {
        missing.push(maximum_field.to_string());
        return Value::Null;
    }
    let cuts: Vec<f64> = skill
        .settings
        .get("zone")
        .map(|values| values.iter().filter_map(|value| value.parse().ok()).collect())
        .unwrap_or_else(|| vec![0.6, 0.7, 0.8, 0.9]);
    let mut seconds = vec![0.0; cuts.len() + 1];
    for (bpm, duration_ms) in durations(&samples(sample_value.unwrap_or(&Value::Null))) {
        let fraction = bpm / maximum;
        let mut bucket = cuts.len();
        for (index, cut) in cuts.iter().enumerate() {
            if fraction < *cut {
                bucket = index;
                break;
            }
        }
        seconds[bucket] += duration_ms as f64 / 1000.0;
    }
    json!({
        "seconds": seconds,
    })
}

fn recovery(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let field = setting(skill, "sample_field").unwrap_or("heart_rate_samples");
    let later_ms: i64 = setting(skill, "later_offset_ms").and_then(|value| value.parse().ok()).unwrap_or(60_000);
    let Some(value) = lookup(&record.data, field, fields_read) else {
        missing.push(field.to_string());
        return Value::Null;
    };
    let points = samples(value);
    let Some((end_offset, end_bpm)) = points.iter().max_by_key(|(offset, _)| *offset).copied() else {
        missing.push(field.to_string());
        return Value::Null;
    };
    let target = end_offset + later_ms;
    let Some((_, later_bpm)) = points.iter().find(|(offset, _)| *offset == target) else {
        missing.push(format!("{field} at offset {target}"));
        return Value::Null;
    };
    json!({ "bpm": end_bpm - later_bpm })
}

fn resting(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let field = setting(skill, "sample_field").unwrap_or("heart_rate_samples");
    let sustain: i64 = setting(skill, "sustain_ms").and_then(|value| value.parse().ok()).unwrap_or(60_000);
    let Some(value) = lookup(&record.data, field, fields_read) else {
        missing.push(field.to_string());
        return Value::Null;
    };
    let points = samples(value);
    let mut best: Option<f64> = None;
    for (start_index, _) in points.iter().enumerate() {
        let start = points[start_index].0;
        let window: Vec<f64> = points
            .iter()
            .filter(|(offset, _)| *offset >= start && *offset <= start + sustain)
            .map(|(_, bpm)| *bpm)
            .collect();
        let covered = points
            .iter()
            .filter(|(offset, _)| *offset >= start && *offset <= start + sustain)
            .map(|(offset, _)| *offset)
            .max()
            .unwrap_or(start)
            - start;
        if covered < sustain || window.is_empty() {
            continue;
        }
        let high = window.iter().copied().fold(f64::MIN, f64::max);
        let low = window.iter().copied().fold(f64::MAX, f64::min);
        if (high - low).abs() < 0.001 {
            best = Some(best.map(|current| current.min(low)).unwrap_or(low));
        }
    }
    let Some(bpm) = best else {
        missing.push(format!("{field} held for {sustain} ms"));
        return Value::Null;
    };
    json!({ "bpm": bpm })
}

fn load_hr(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let sample_field = setting(skill, "sample_field").unwrap_or("heart_rate_samples");
    let rest_field = setting(skill, "rest_field").unwrap_or("resting_heart_rate_bpm");
    let maximum_field = setting(skill, "maximum_field").unwrap_or("maximum_heart_rate_bpm");
    let factor: f64 = setting(skill, "factor").and_then(|value| value.parse().ok()).unwrap_or(1.92);
    let samples_value = lookup(&record.data, sample_field, fields_read);
    let rest = lookup(&record.data, rest_field, fields_read).and_then(Value::as_f64);
    let maximum = lookup(&record.data, maximum_field, fields_read).and_then(Value::as_f64);
    if samples_value.is_none() {
        missing.push(sample_field.to_string());
    }
    if rest.is_none() {
        missing.push(rest_field.to_string());
    }
    if maximum.is_none() {
        missing.push(maximum_field.to_string());
    }
    let (Some(rest), Some(maximum), Some(samples_value)) = (rest, maximum, samples_value) else {
        return Value::Null;
    };
    if maximum <= rest {
        missing.push(format!("{maximum_field} is not above {rest_field}"));
        return Value::Null;
    }
    let mut load = 0.0;
    for (bpm, duration_ms) in durations(&samples(samples_value)) {
        let reserve = ((bpm - rest) / (maximum - rest)).clamp(0.0, 1.0);
        let minutes = duration_ms as f64 / 60_000.0;
        load += minutes * reserve * (factor * reserve).exp();
    }
    json!({ "load": (load * 1000.0).round() / 1000.0 })
}

fn load_effort(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let duration_field = setting(skill, "duration_field").unwrap_or("duration_seconds");
    let effort_field = setting(skill, "effort_field").unwrap_or("rating");
    let duration = lookup(&record.data, duration_field, fields_read).and_then(Value::as_f64);
    let effort = lookup(&record.data, effort_field, fields_read).and_then(Value::as_f64);
    if duration.is_none() {
        missing.push(duration_field.to_string());
    }
    if effort.is_none() {
        missing.push(effort_field.to_string());
    }
    let (Some(duration), Some(effort)) = (duration, effort) else {
        return Value::Null;
    };
    json!({ "load": (duration / 60.0) * effort })
}

fn number_field(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let field = setting(skill, "field").unwrap_or("asleep_seconds");
    match lookup(&record.data, field, fields_read).and_then(Value::as_f64) {
        Some(value) => json!({ "value": value }),
        None => {
            missing.push(field.to_string());
            Value::Null
        }
    }
}

fn efficiency(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let asleep_field = setting(skill, "asleep_field").unwrap_or("asleep_seconds");
    let in_bed_field = setting(skill, "in_bed_field").unwrap_or("in_bed_seconds");
    let asleep = lookup(&record.data, asleep_field, fields_read).and_then(Value::as_f64);
    let in_bed = lookup(&record.data, in_bed_field, fields_read).and_then(Value::as_f64);
    if asleep.is_none() {
        missing.push(asleep_field.to_string());
    }
    if in_bed.is_none() {
        missing.push(in_bed_field.to_string());
    }
    let (Some(asleep), Some(in_bed)) = (asleep, in_bed) else {
        return Value::Null;
    };
    if in_bed == 0.0 {
        missing.push(in_bed_field.to_string());
        return Value::Null;
    }
    json!({ "efficiency": asleep / in_bed })
}

fn timing(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let start_field = setting(skill, "start_field").unwrap_or("start_at");
    let end_field = setting(skill, "end_field").unwrap_or("end_at");
    let start = lookup(&record.data, start_field, fields_read).and_then(Value::as_str).map(str::to_string);
    let end = lookup(&record.data, end_field, fields_read).and_then(Value::as_str).map(str::to_string);
    if start.is_none() {
        missing.push(start_field.to_string());
    }
    if end.is_none() {
        missing.push(end_field.to_string());
    }
    let (Some(start), Some(end)) = (start, end) else {
        return Value::Null;
    };
    let start_ms = parse_millis(&start);
    let end_ms = parse_millis(&end);
    let midpoint = match (start_ms, end_ms) {
        (Some(start_ms), Some(end_ms)) => {
            Some(crate::time::format_millis(start_ms + (end_ms - start_ms) / 2))
        }
        _ => None,
    };
    json!({ "start_at": start, "midpoint_at": midpoint })
}

fn stages(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let field = setting(skill, "field").unwrap_or("stage_minutes");
    let Some(value) = lookup(&record.data, field, fields_read).and_then(Value::as_object) else {
        missing.push(field.to_string());
        return Value::Null;
    };
    let mut total = 0.0;
    let mut named = Map::new();
    for (name, minutes) in value {
        if let Some(minutes) = minutes.as_f64() {
            named.insert(name.clone(), json!(minutes));
            total += minutes;
        }
    }
    json!({ "stages": named, "total_minutes": total })
}

fn regularity(
    skill: &SkillFile,
    record: &StoredRecord,
    related: &[StoredRecord],
    fields_read: &mut Vec<Value>,
    missing: &mut Vec<String>,
) -> Value {
    let minimum: usize = setting(skill, "minimum_nights")
        .and_then(|value| value.parse().ok())
        .unwrap_or(7);
    fields_read.push(json!({"field": "asleep window", "present": true}));
    let mut nights: Vec<&StoredRecord> = related.iter().chain(std::iter::once(record)).collect();
    nights.sort_by(|left, right| {
        left.data
            .get("start_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.data.get("start_at").and_then(Value::as_str).unwrap_or(""))
    });
    nights.dedup_by(|left, right| left.record_id == right.record_id);
    if nights.len() < minimum {
        missing.push(format!("found {} nights, need {minimum}", nights.len()));
        return Value::Null;
    }
    let mut agreements = Vec::new();
    for pair in nights.windows(2) {
        agreements.push(night_agreement(&pair[0].data, &pair[1].data));
    }
    let mean = agreements.iter().sum::<f64>() / agreements.len() as f64;
    json!({ "nights": nights.len(), "regularity": (mean * 1000.0).round() / 1000.0 })
}

fn night_agreement(left: &Value, right: &Value) -> f64 {
    let Some((left_start, left_end)) = asleep_bounds(left) else {
        return 0.0;
    };
    let Some((right_start, right_end)) = asleep_bounds(right) else {
        return 0.0;
    };
    let start = left_start.min(right_start);
    let end = left_end.max(right_end).max(start + 86_400_000);
    let mut agree = 0.0;
    let mut total = 0.0;
    let mut cursor = start;
    while cursor < end {
        let left_asleep = cursor >= left_start && cursor < left_end;
        let right_asleep = cursor >= right_start && cursor < right_end;
        if left_asleep == right_asleep {
            agree += 1.0;
        }
        total += 1.0;
        cursor += 60_000;
    }
    if total == 0.0 { 0.0 } else { agree / total }
}

fn asleep_bounds(data: &Value) -> Option<(i64, i64)> {
    let start = parse_millis(data.get("start_at")?.as_str()?)?;
    let seconds = data.get("asleep_seconds").and_then(Value::as_f64)?;
    Some((start, start + (seconds * 1000.0) as i64))
}

fn volume(skill: &SkillFile, record: &StoredRecord, fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let field = setting(skill, "field").unwrap_or("sets");
    let Some(sets) = lookup(&record.data, field, fields_read).and_then(Value::as_array) else {
        missing.push(field.to_string());
        if record.data.get("text_result").is_some() {
            missing.push("text_result is text, so volume cannot be made".into());
        }
        return Value::Null;
    };
    if sets.is_empty() {
        missing.push(field.to_string());
        return Value::Null;
    }
    let mut total = 0.0;
    let mut unit = None;
    for set in sets {
        let reps = set.get("reps").and_then(Value::as_f64);
        let weight = set.get("weight").and_then(Value::as_f64);
        let set_unit = set.get("weight_unit").and_then(Value::as_str);
        let (Some(reps), Some(weight), Some(set_unit)) = (reps, weight, set_unit) else {
            missing.push(field.to_string());
            return Value::Null;
        };
        if unit.is_none() {
            unit = Some(set_unit.to_string());
        }
        total += reps * weight;
    }
    json!({ "volume": total, "weight_unit": unit })
}

fn quantity_change(record: &StoredRecord, related: &[StoredRecord], fields_read: &mut Vec<Value>, missing: &mut Vec<String>) -> Value {
    let quantity = record.data.get("quantity").and_then(Value::as_str).unwrap_or("");
    fields_read.push(json!({"field": "quantity", "present": !quantity.is_empty(), "value": quantity}));
    let value = record
        .data
        .get("value")
        .and_then(|value| value.get("standard_value"))
        .and_then(Value::as_f64);
    let Some(value) = value else {
        missing.push("value.standard_value".into());
        return Value::Null;
    };
    fields_read.push(json!({"field": "value.standard_value", "present": true}));
    let earlier: Vec<f64> = related
        .iter()
        .filter(|other| other.record_id != record.record_id)
        .filter(|other| other.data.get("quantity").and_then(Value::as_str) == Some(quantity))
        .filter_map(|other| {
            other
                .data
                .get("value")
                .and_then(|value| value.get("standard_value"))
                .and_then(Value::as_f64)
        })
        .collect();
    if earlier.is_empty() {
        missing.push(format!("earlier {quantity} records"));
        return Value::Null;
    }
    let mean = earlier.iter().sum::<f64>() / earlier.len() as f64;
    json!({
        "quantity": quantity,
        "change": ((value - mean) * 1000.0).round() / 1000.0,
        "compared_records": earlier.len(),
    })
}

fn sleep_change(
    skill: &SkillFile,
    record: &StoredRecord,
    related: &[StoredRecord],
    fields_read: &mut Vec<Value>,
    missing: &mut Vec<String>,
) -> Value {
    let field = setting(skill, "field").unwrap_or("asleep_seconds");
    let Some(value) = lookup(&record.data, field, fields_read).and_then(Value::as_f64) else {
        missing.push(field.to_string());
        return Value::Null;
    };
    let earlier: Vec<f64> = related
        .iter()
        .filter(|other| other.record_id != record.record_id)
        .filter_map(|other| other.data.get(field).and_then(Value::as_f64))
        .collect();
    if earlier.is_empty() {
        missing.push(format!("earlier {field}"));
        return Value::Null;
    }
    let mean = earlier.iter().sum::<f64>() / earlier.len() as f64;
    json!({ "change": value - mean, "compared_records": earlier.len() })
}

#[cfg(test)]
mod tests {
    use super::{parse_skill, run_skill};
    use crate::store::StoredRecord;
    use serde_json::json;

    #[test]
    fn zones_match_the_worked_example() {
        let skill = parse_skill(include_str!("../../../skills/heart-rate-zones/SKILL.md")).unwrap();
        let record = StoredRecord {
            record_id: "018f0000-0000-7000-8000-000000000001".into(),
            record_type: "workout_session".into(),
            revision: 1,
            data: json!({
                "maximum_heart_rate_bpm": 200,
                "heart_rate_samples": [
                    {"offset_ms": 0, "bpm": 100},
                    {"offset_ms": 1000, "bpm": 130},
                    {"offset_ms": 2000, "bpm": 160},
                    {"offset_ms": 3000, "bpm": 190}
                ]
            }),
        };
        let result = run_skill(&skill, &record, &[]);
        assert_eq!(result["result"]["seconds"], json!([1.0, 1.0, 0.0, 1.0, 1.0]));
        assert!(result["missing"].as_array().unwrap().is_empty());
    }
}

fn parse_millis(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|value| value.timestamp_millis())
}

