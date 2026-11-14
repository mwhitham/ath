//! `ath apply`: one calculation file on one record.
//!
//! The calculation reads named fields of the record's current stored data.
//! A field written `supplied:<name>` reads a value kept under the name its
//! source supplied. Fields that are absent are listed in `missing` and the
//! result is null; nothing is guessed. Some calculations also read other
//! current records of the same source. Nothing is written.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::definition::{CalculationFile, OneRecordCalculation};
use crate::error::{Error, Result};
use crate::json::{self, Map, Value};
use crate::model::time;

/// A record's current stored data.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRecord {
    pub record_id: String,
    pub record_type: String,
    pub revision: u64,
    pub data: Value,
}

/// One field a calculation looked for.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldRead {
    pub field: String,
    pub present: bool,
    /// The quantity read by a quantity change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The value a one-record calculation produced.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum OneRecordResult {
    Zones { seconds: Vec<f64> },
    HeartRate { bpm: f64 },
    Load { load: f64 },
    Value { value: f64 },
    Efficiency { efficiency: f64 },
    Timing { start_at: String, midpoint_at: Option<String> },
    Stages { stages: Map<String, Value>, total_minutes: f64 },
    Regularity { nights: usize, regularity: f64 },
    Volume { volume: f64, weight_unit: Option<String> },
    QuantityChange { quantity: String, change: f64, compared_records: usize },
    Change { change: f64, compared_records: usize },
}

/// The `ath apply --json` object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApplyResult {
    pub calculation: String,
    pub record_id: String,
    pub fields_read: Vec<FieldRead>,
    pub missing: Vec<String>,
    /// Null when an input is missing.
    pub result: Option<OneRecordResult>,
}

/// A current, not deleted record's stored data.
pub fn stored_record(connection: &Connection, record_id: &str) -> Result<StoredRecord> {
    let row: Option<(String, String, i64, String)> = connection
        .query_row(
            "SELECT records.record_id, records.record_type, records.current_revision, record_revisions.record_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1 AND records.is_deleted = 0",
            [record_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((record_id, record_type, revision, record_json)) = row else {
        return Err(Error::not_found(format!("record {record_id} was not found: Query returned no rows")));
    };
    Ok(StoredRecord { record_id, record_type, revision: revision as u64, data: json::parse(&record_json)? })
}

fn stored_records(connection: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<StoredRecord>> {
    let mut statement = connection.prepare(sql)?;
    let ids = statement.query_map(args, |row| row.get::<_, String>(0))?.collect::<std::result::Result<Vec<_>, _>>()?;
    ids.iter().map(|id| stored_record(connection, id)).collect()
}

/// The other current records a change or regularity calculation compares
/// against: same-source point measurements of the same quantity, or
/// same-source sessions of the same type. The record itself is included.
pub fn related_records(connection: &Connection, record: &StoredRecord) -> Result<Vec<StoredRecord>> {
    let Some(source_id) = record.data.get("source_id").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };
    match record.record_type.as_str() {
        "point_measurement" => {
            let quantity = record.data.get("quantity").and_then(Value::as_str).unwrap_or("");
            stored_records(
                connection,
                "SELECT records.record_id
                 FROM point_measurements
                 JOIN records ON records.record_id = point_measurements.record_id
                 WHERE point_measurements.source_id = ?1
                   AND point_measurements.quantity = ?2
                   AND records.is_deleted = 0
                 ORDER BY point_measurements.recorded_at, records.record_id",
                params![source_id, quantity],
            )
        }
        "sleep_session" | "workout_session" => {
            let table = if record.record_type == "sleep_session" { "sleep_sessions" } else { "workout_sessions" };
            stored_records(
                connection,
                &format!(
                    "SELECT records.record_id
                     FROM {table}
                     JOIN records ON records.record_id = {table}.record_id
                     WHERE {table}.source_id = ?1 AND records.is_deleted = 0
                     ORDER BY records.created_at, records.record_id"
                ),
                params![source_id],
            )
        }
        _ => Ok(Vec::new()),
    }
}

fn uses_related(calculation: &OneRecordCalculation) -> bool {
    matches!(
        calculation,
        OneRecordCalculation::NightAgreement { .. }
            | OneRecordCalculation::QuantityChange
            | OneRecordCalculation::SleepChange { .. }
    )
}

/// Run a calculation file on one current record.
pub fn apply(connection: &Connection, file: &CalculationFile, record_id: &str) -> Result<ApplyResult> {
    apply_to(connection, file, stored_record(connection, record_id)?)
}

/// Run a calculation file on a record already read.
pub fn apply_to(connection: &Connection, file: &CalculationFile, record: StoredRecord) -> Result<ApplyResult> {
    let related = if uses_related(&file.calculation) { related_records(connection, &record)? } else { Vec::new() };
    Ok(run(file, &record, &related))
}

/// Run a calculation on a record and the records it compares against.
pub fn run(file: &CalculationFile, record: &StoredRecord, related: &[StoredRecord]) -> ApplyResult {
    let mut context = Context { data: &record.data, fields_read: Vec::new(), missing: Vec::new() };
    use OneRecordCalculation::*;
    let result = match &file.calculation {
        TimeBetweenCuts { sample_field, maximum_field, cuts } => context.zones(sample_field, maximum_field, cuts),
        ValueAfterEnd { sample_field, later_offset_ms } => context.recovery(sample_field, *later_offset_ms),
        LowestHeld { sample_field, sustain_ms } => context.resting(sample_field, *sustain_ms),
        HeartRateReserveCurve { sample_field, rest_field, maximum_field, factor } => {
            context.load_heart_rate(sample_field, rest_field, maximum_field, *factor)
        }
        Multiply { duration_field, effort_field } => context.load_effort(duration_field, effort_field),
        Field { field } => context.number_field(field),
        Divide { asleep_field, in_bed_field } => context.efficiency(asleep_field, in_bed_field),
        Midpoint { start_field, end_field } => context.timing(start_field, end_field),
        SumFields { field } => context.stages(field),
        NightAgreement { minimum_nights } => context.regularity(*minimum_nights, record, related),
        RepsTimesWeight { field } => context.volume(field),
        QuantityChange => context.quantity_change(record, related),
        SleepChange { field } => context.sleep_change(field, record, related),
        Unknown(name) => {
            context.missing.push(format!("unknown calculation {name}"));
            None
        }
    };
    ApplyResult {
        calculation: file.name.clone(),
        record_id: record.record_id.clone(),
        fields_read: context.fields_read,
        missing: context.missing,
        result,
    }
}

struct Context<'a> {
    data: &'a Value,
    fields_read: Vec<FieldRead>,
    missing: Vec<String>,
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
    let mut out = Vec::with_capacity(samples.len());
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

/// `chrono::DateTime::parse_from_rfc3339`, as the 2.1.0 calculations read times.
pub fn parse_millis(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text).ok().map(|value| value.timestamp_millis())
}

/// Rounded to three decimal places, halves away from zero.
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

impl<'a> Context<'a> {
    fn lookup(&mut self, field: &str) -> Option<&'a Value> {
        let value = match field.strip_prefix("supplied:") {
            Some(name) => self.data.get("supplied_fields").and_then(|fields| fields.get(name)),
            None => self.data.get(field),
        };
        self.fields_read.push(FieldRead { field: field.to_string(), present: value.is_some(), value: None });
        value
    }

    fn zones(&mut self, sample_field: &str, maximum_field: &str, cuts: &[f64]) -> Option<OneRecordResult> {
        let sample_value = self.lookup(sample_field);
        let maximum = self.lookup(maximum_field).and_then(Value::as_f64);
        if sample_value.is_none() {
            self.missing.push(sample_field.to_string());
        }
        let Some(maximum) = maximum else {
            self.missing.push(maximum_field.to_string());
            return None;
        };
        if maximum <= 0.0 {
            self.missing.push(maximum_field.to_string());
            return None;
        }
        let mut seconds = vec![0.0; cuts.len() + 1];
        for (bpm, duration_ms) in durations(&samples(sample_value.unwrap_or(&Value::Null))) {
            let fraction = bpm / maximum;
            let bucket = cuts.iter().position(|cut| fraction < *cut).unwrap_or(cuts.len());
            seconds[bucket] += duration_ms as f64 / 1000.0;
        }
        Some(OneRecordResult::Zones { seconds })
    }

    fn recovery(&mut self, field: &str, later_ms: i64) -> Option<OneRecordResult> {
        let Some(value) = self.lookup(field) else {
            self.missing.push(field.to_string());
            return None;
        };
        let points = samples(value);
        let Some((end_offset, end_bpm)) = points.iter().max_by_key(|(offset, _)| *offset).copied() else {
            self.missing.push(field.to_string());
            return None;
        };
        let target = end_offset.wrapping_add(later_ms);
        let Some((_, later_bpm)) = points.iter().find(|(offset, _)| *offset == target) else {
            self.missing.push(format!("{field} at offset {target}"));
            return None;
        };
        Some(OneRecordResult::HeartRate { bpm: end_bpm - later_bpm })
    }

    fn resting(&mut self, field: &str, sustain: i64) -> Option<OneRecordResult> {
        let Some(value) = self.lookup(field) else {
            self.missing.push(field.to_string());
            return None;
        };
        let points = samples(value);
        let mut best: Option<f64> = None;
        for &(start, _) in &points {
            let end = start.wrapping_add(sustain);
            let inside = || points.iter().filter(move |(offset, _)| *offset >= start && *offset <= end);
            let window: Vec<f64> = inside().map(|(_, bpm)| *bpm).collect();
            let covered = inside().map(|(offset, _)| *offset).max().unwrap_or(start) - start;
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
            self.missing.push(format!("{field} held for {sustain} ms"));
            return None;
        };
        Some(OneRecordResult::HeartRate { bpm })
    }

    fn load_heart_rate(
        &mut self,
        sample_field: &str,
        rest_field: &str,
        maximum_field: &str,
        factor: f64,
    ) -> Option<OneRecordResult> {
        let samples_value = self.lookup(sample_field);
        let rest = self.lookup(rest_field).and_then(Value::as_f64);
        let maximum = self.lookup(maximum_field).and_then(Value::as_f64);
        if samples_value.is_none() {
            self.missing.push(sample_field.to_string());
        }
        if rest.is_none() {
            self.missing.push(rest_field.to_string());
        }
        if maximum.is_none() {
            self.missing.push(maximum_field.to_string());
        }
        let (Some(rest), Some(maximum), Some(samples_value)) = (rest, maximum, samples_value) else {
            return None;
        };
        if maximum <= rest {
            self.missing.push(format!("{maximum_field} is not above {rest_field}"));
            return None;
        }
        let mut load = 0.0;
        for (bpm, duration_ms) in durations(&samples(samples_value)) {
            let reserve = ((bpm - rest) / (maximum - rest)).clamp(0.0, 1.0);
            let minutes = duration_ms as f64 / 60_000.0;
            load += minutes * reserve * (factor * reserve).exp();
        }
        Some(OneRecordResult::Load { load: round3(load) })
    }

    fn load_effort(&mut self, duration_field: &str, effort_field: &str) -> Option<OneRecordResult> {
        let duration = self.lookup(duration_field).and_then(Value::as_f64);
        let effort = self.lookup(effort_field).and_then(Value::as_f64);
        if duration.is_none() {
            self.missing.push(duration_field.to_string());
        }
        if effort.is_none() {
            self.missing.push(effort_field.to_string());
        }
        let (Some(duration), Some(effort)) = (duration, effort) else {
            return None;
        };
        Some(OneRecordResult::Load { load: (duration / 60.0) * effort })
    }

    fn number_field(&mut self, field: &str) -> Option<OneRecordResult> {
        match self.lookup(field).and_then(Value::as_f64) {
            Some(value) => Some(OneRecordResult::Value { value }),
            None => {
                self.missing.push(field.to_string());
                None
            }
        }
    }

    fn efficiency(&mut self, asleep_field: &str, in_bed_field: &str) -> Option<OneRecordResult> {
        let asleep = self.lookup(asleep_field).and_then(Value::as_f64);
        let in_bed = self.lookup(in_bed_field).and_then(Value::as_f64);
        if asleep.is_none() {
            self.missing.push(asleep_field.to_string());
        }
        if in_bed.is_none() {
            self.missing.push(in_bed_field.to_string());
        }
        let (Some(asleep), Some(in_bed)) = (asleep, in_bed) else {
            return None;
        };
        if in_bed == 0.0 {
            self.missing.push(in_bed_field.to_string());
            return None;
        }
        Some(OneRecordResult::Efficiency { efficiency: asleep / in_bed })
    }

    fn timing(&mut self, start_field: &str, end_field: &str) -> Option<OneRecordResult> {
        let start = self.lookup(start_field).and_then(Value::as_str).map(str::to_string);
        let end = self.lookup(end_field).and_then(Value::as_str).map(str::to_string);
        if start.is_none() {
            self.missing.push(start_field.to_string());
        }
        if end.is_none() {
            self.missing.push(end_field.to_string());
        }
        let (Some(start), Some(end)) = (start, end) else {
            return None;
        };
        let midpoint = match (parse_millis(&start), parse_millis(&end)) {
            (Some(start_ms), Some(end_ms)) => Some(time::format_utc_millis(start_ms + (end_ms - start_ms) / 2)),
            _ => None,
        };
        Some(OneRecordResult::Timing { start_at: start, midpoint_at: midpoint })
    }

    fn stages(&mut self, field: &str) -> Option<OneRecordResult> {
        let Some(value) = self.lookup(field).and_then(Value::as_object) else {
            self.missing.push(field.to_string());
            return None;
        };
        let mut total = 0.0;
        let mut named = Map::new();
        for (name, minutes) in value {
            if let Some(minutes) = minutes.as_f64() {
                named.insert(name.clone(), json::to_value(&minutes));
                total += minutes;
            }
        }
        Some(OneRecordResult::Stages { stages: named, total_minutes: total })
    }

    fn regularity(
        &mut self,
        minimum: usize,
        record: &StoredRecord,
        related: &[StoredRecord],
    ) -> Option<OneRecordResult> {
        self.fields_read.push(FieldRead { field: "asleep window".into(), present: true, value: None });
        let mut nights: Vec<&StoredRecord> = related.iter().chain(std::iter::once(record)).collect();
        let start = |night: &StoredRecord| night.data.get("start_at").and_then(Value::as_str).unwrap_or("").to_string();
        nights.sort_by_key(|night| start(night));
        nights.dedup_by(|left, right| left.record_id == right.record_id);
        if nights.len() < minimum {
            self.missing.push(format!("found {} nights, need {minimum}", nights.len()));
            return None;
        }
        let agreements: Vec<f64> =
            nights.windows(2).map(|pair| night_agreement(&pair[0].data, &pair[1].data)).collect();
        let mean = agreements.iter().sum::<f64>() / agreements.len() as f64;
        Some(OneRecordResult::Regularity { nights: nights.len(), regularity: round3(mean) })
    }

    fn volume(&mut self, field: &str) -> Option<OneRecordResult> {
        let Some(sets) = self.lookup(field).and_then(Value::as_array) else {
            self.missing.push(field.to_string());
            if self.data.get("text_result").is_some() {
                self.missing.push("text_result is text, so volume cannot be made".into());
            }
            return None;
        };
        if sets.is_empty() {
            self.missing.push(field.to_string());
            return None;
        }
        let mut total = 0.0;
        let mut unit = None;
        for set in sets {
            let reps = set.get("reps").and_then(Value::as_f64);
            let weight = set.get("weight").and_then(Value::as_f64);
            let set_unit = set.get("weight_unit").and_then(Value::as_str);
            let (Some(reps), Some(weight), Some(set_unit)) = (reps, weight, set_unit) else {
                self.missing.push(field.to_string());
                return None;
            };
            if unit.is_none() {
                unit = Some(set_unit.to_string());
            }
            total += reps * weight;
        }
        Some(OneRecordResult::Volume { volume: total, weight_unit: unit })
    }

    fn quantity_change(&mut self, record: &StoredRecord, related: &[StoredRecord]) -> Option<OneRecordResult> {
        let quantity = record.data.get("quantity").and_then(Value::as_str).unwrap_or("");
        self.fields_read.push(FieldRead {
            field: "quantity".into(),
            present: !quantity.is_empty(),
            value: Some(quantity.to_string()),
        });
        let standard =
            |data: &Value| data.get("value").and_then(|value| value.get("standard_value")).and_then(Value::as_f64);
        let Some(value) = standard(&record.data) else {
            self.missing.push("value.standard_value".into());
            return None;
        };
        self.fields_read.push(FieldRead { field: "value.standard_value".into(), present: true, value: None });
        // The 2.1.0 arithmetic compares with every other same-source record of
        // the quantity, including later ones.
        let earlier: Vec<f64> = related
            .iter()
            .filter(|other| other.record_id != record.record_id)
            .filter(|other| other.data.get("quantity").and_then(Value::as_str) == Some(quantity))
            .filter_map(|other| standard(&other.data))
            .collect();
        if earlier.is_empty() {
            self.missing.push(format!("earlier {quantity} records"));
            return None;
        }
        let mean = earlier.iter().sum::<f64>() / earlier.len() as f64;
        Some(OneRecordResult::QuantityChange {
            quantity: quantity.to_string(),
            change: round3(value - mean),
            compared_records: earlier.len(),
        })
    }

    fn sleep_change(
        &mut self,
        field: &str,
        record: &StoredRecord,
        related: &[StoredRecord],
    ) -> Option<OneRecordResult> {
        let Some(value) = self.lookup(field).and_then(Value::as_f64) else {
            self.missing.push(field.to_string());
            return None;
        };
        let earlier: Vec<f64> = related
            .iter()
            .filter(|other| other.record_id != record.record_id)
            .filter_map(|other| other.data.get(field).and_then(Value::as_f64))
            .collect();
        if earlier.is_empty() {
            self.missing.push(format!("earlier {field}"));
            return None;
        }
        let mean = earlier.iter().sum::<f64>() / earlier.len() as f64;
        Some(OneRecordResult::Change { change: value - mean, compared_records: earlier.len() })
    }
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
    if total == 0.0 {
        0.0
    } else {
        agree / total
    }
}

fn asleep_bounds(data: &Value) -> Option<(i64, i64)> {
    let start = parse_millis(data.get("start_at")?.as_str()?)?;
    let seconds = data.get("asleep_seconds").and_then(Value::as_f64)?;
    Some((start, start + (seconds * 1000.0) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calculate::definition::shipped_skill_file;

    fn record(data: Value) -> StoredRecord {
        StoredRecord {
            record_id: "018f0000-0000-7000-8000-000000000001".into(),
            record_type: "workout_session".into(),
            revision: 1,
            data,
        }
    }

    #[test]
    fn zones_match_the_worked_example() {
        let file = CalculationFile::parse(shipped_skill_file("heart-rate-zones").unwrap()).unwrap();
        let data = json::parse(
            r#"{"maximum_heart_rate_bpm":200,"heart_rate_samples":[
                {"offset_ms":0,"bpm":100},{"offset_ms":1000,"bpm":130},
                {"offset_ms":2000,"bpm":160},{"offset_ms":3000,"bpm":190}]}"#,
        )
        .unwrap();
        let result = run(&file, &record(data), &[]);
        assert_eq!(result.result, Some(OneRecordResult::Zones { seconds: vec![1.0, 1.0, 0.0, 1.0, 1.0] }));
        assert!(result.missing.is_empty());
        assert_eq!(
            json::encode(&result),
            r#"{"calculation":"heart-rate-zones","record_id":"018f0000-0000-7000-8000-000000000001","fields_read":[{"field":"heart_rate_samples","present":true},{"field":"maximum_heart_rate_bpm","present":true}],"missing":[],"result":{"seconds":[1,1,0,1,1]}}"#
        );
    }

    #[test]
    fn a_missing_maximum_is_reported_not_guessed() {
        let file = CalculationFile::parse(shipped_skill_file("heart-rate-zones").unwrap()).unwrap();
        let result = run(&file, &record(json::parse(r#"{"heart_rate_samples":[]}"#).unwrap()), &[]);
        assert_eq!(result.missing, vec!["maximum_heart_rate_bpm".to_string()]);
        assert_eq!(json::encode(&result.result), "null");
    }
}
