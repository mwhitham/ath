//! Import candidates: what a provider reader found, before matching.
//!
//! A candidate is one complete record a reader wants stored, plus the key that
//! finds it again on a later import. The import operation decides whether it
//! is new, already stored, a correction, or a conflict. Readers never issue SQL.
//!
//! The helpers below build the receipts every provider route shares. They are
//! the rules of `src/db/provider-import.ts` in 2.1.0.

use crate::json::{self, Value};
use crate::model::*;

/// One part of a match key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyPart {
    /// Literal text, such as a record type, a quantity, or a time.
    Text(String),
    /// The ID of the candidate's source, filled in after source resolution.
    Source,
}

impl KeyPart {
    pub fn text(value: impl Into<String>) -> Self {
        KeyPart::Text(value.into())
    }
}

/// How a candidate is found again on a later import.
///
/// The stored key is the JSON text of the parts, such as
/// `["point_measurement","<source id>","hrv_rmssd","2026-08-09T13:05:00.000Z",0]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchKey {
    pub parts: Vec<KeyPart>,
    /// When true, the operation appends the candidate's position among
    /// candidates with the same parts: 0, then 1, and so on. Candidates that
    /// share parts form a collision group. A changed record in a group of two
    /// or more is a conflict, because the importer cannot tell which row was
    /// corrected.
    pub numbered: bool,
    /// When true, the key is the text of its one part, not a JSON array.
    pub literal: bool,
}

impl MatchKey {
    /// A key for records an export may repeat, such as two identical rows.
    pub fn numbered(parts: Vec<KeyPart>) -> Self {
        MatchKey { parts, numbered: true, literal: false }
    }

    /// A key an export gives once, such as a device.
    pub fn unique(parts: Vec<KeyPart>) -> Self {
        MatchKey { parts, numbered: false, literal: false }
    }

    /// A key stored as this exact text. The keyed routes used plain text keys
    /// in 2.1.0: a Garmin activity ID or file name, a Strava activity ID, and
    /// the escaped parts of a mapped key (D181). It has no number and no
    /// collision group.
    pub fn literal(key: impl Into<String>) -> Self {
        MatchKey { parts: vec![KeyPart::Text(key.into())], numbered: false, literal: true }
    }

    /// `[record_type, source, discriminator, anchor]`: the key of every measured
    /// record in the 2.1.0 provider routes.
    pub fn measured(record_type: RecordType, discriminator: &str, anchor: &str) -> Self {
        MatchKey::numbered(vec![
            KeyPart::text(record_type.as_str()),
            KeyPart::Source,
            KeyPart::text(discriminator),
            KeyPart::text(anchor),
        ])
    }

    /// `[record_type, source, start_at]`: sleep and workout sessions.
    pub fn session(record_type: RecordType, start_at: &str) -> Self {
        MatchKey::numbered(vec![KeyPart::text(record_type.as_str()), KeyPart::Source, KeyPart::text(start_at)])
    }

    /// `["athlete_report", report kind, reported_at]`: athlete reports and
    /// nutrition entries. They have no source, so two devices cannot split them.
    pub fn athlete_report(kind: &str, reported_at: &str) -> Self {
        MatchKey::numbered(vec![KeyPart::text("athlete_report"), KeyPart::text(kind), KeyPart::text(reported_at)])
    }
}

/// Where a candidate sits in the plan. The plan lists sources, then devices,
/// then records, provider summaries, samples, and athlete reports, as 2.1.0
/// did. Within a section, candidates keep the order the reader sent them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Device = 1,
    Record = 2,
    Summary = 3,
    Sample = 4,
    Report = 5,
}

/// One record a reader found.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The complete record data. A measured record or device carries the source
    /// ID that `CandidateSink::source` returned.
    pub data: RecordData,
    pub key: MatchKey,
    pub section: Section,
}

impl Candidate {
    pub fn new(data: RecordData, key: MatchKey, section: Section) -> Self {
        Candidate { data, key, section }
    }

    /// Route points this record would store.
    pub fn route_points(&self) -> u64 {
        match &self.data {
            RecordData::WorkoutSession(workout) => workout.route.as_ref().map_or(0, |route| route.len() as u64),
            _ => 0,
        }
    }
}

// ---- Receipts -------------------------------------------------------------

/// The trailing `Z` or `+HH:MM` of provider text, upper-cased.
pub fn offset_text(timestamp: &str) -> Option<String> {
    let bytes = timestamp.as_bytes();
    if bytes.last().is_some_and(|byte| byte.eq_ignore_ascii_case(&b'z')) {
        return Some("Z".into());
    }
    if bytes.len() >= 6 {
        let tail = &bytes[bytes.len() - 6..];
        if matches!(tail[0], b'+' | b'-')
            && tail[1].is_ascii_digit()
            && tail[2].is_ascii_digit()
            && tail[3] == b':'
            && tail[4].is_ascii_digit()
            && tail[5].is_ascii_digit()
        {
            return Some(String::from_utf8_lossy(tail).into_owned());
        }
    }
    None
}

/// The receipt 2.1.0 wrote for provider text that already carries its offset.
/// `meaning` is `provider_cycle_offset` for WHOOP and `meaning_not_documented`
/// for the other routes.
pub fn offset_timestamp_receipt(timestamp: &str, meaning: UtcOffsetMeaning) -> InputTimestampReceipt {
    let offset = offset_text(timestamp);
    InputTimestampReceipt {
        timestamp_text: Some(timestamp.to_string()),
        utc_offset_meaning: offset.as_ref().map(|_| meaning),
        utc_offset_missing_reason: offset.is_none().then_some(MissingReason::NotProvidedBySource),
        utc_offset_text: offset,
        source_time_zone: None,
        source_time_zone_meaning: None,
        timestamp_basis: Some(InputTimestampBasis::SourceOffsetTimestamp),
        timestamp_missing_reason: None,
    }
}

/// The receipt for a record whose source gave no time: a date-only provider value.
pub fn missing_timestamp_receipt() -> InputTimestampReceipt {
    InputTimestampReceipt {
        timestamp_text: None,
        utc_offset_text: None,
        utc_offset_meaning: None,
        source_time_zone: None,
        source_time_zone_meaning: None,
        timestamp_basis: None,
        timestamp_missing_reason: Some(MissingReason::NotProvidedBySource),
        utc_offset_missing_reason: Some(MissingReason::NotProvidedBySource),
    }
}

/// What the source wrote for a value, when it differs from the stored value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceInput {
    pub input_value: Option<f64>,
    pub input_value_missing_reason: Option<MissingReason>,
    pub input_unit: Option<String>,
    pub input_unit_missing_reason: Option<MissingReason>,
}

impl SourceInput {
    /// The source wrote this value in this unit.
    pub fn written(value: f64, unit: impl Into<String>) -> Self {
        SourceInput { input_value: Some(value), input_unit: Some(unit.into()), ..SourceInput::default() }
    }

    /// The source wrote this value and named no unit.
    pub fn without_unit(value: f64) -> Self {
        SourceInput {
            input_value: Some(value),
            input_unit_missing_reason: Some(MissingReason::NotProvidedBySource),
            ..SourceInput::default()
        }
    }
}

/// A stored value with what the source wrote. With no `input`, the source
/// wrote the stored value in the stored unit. A missing input value or unit
/// without its own reason is `not_exposed_by_route`.
pub fn source_value(value: f64, unit: &str, input: Option<&SourceInput>) -> SourceValue {
    match input {
        None => SourceValue {
            standard_value: value,
            standard_unit: unit.to_string(),
            input_value: Some(value),
            input_unit: Some(unit.to_string()),
            input_value_missing_reason: None,
            input_unit_missing_reason: None,
        },
        Some(input) => SourceValue {
            standard_value: value,
            standard_unit: unit.to_string(),
            input_value: input.input_value,
            input_value_missing_reason: match input.input_value {
                Some(_) => None,
                None => Some(input.input_value_missing_reason.unwrap_or(MissingReason::NotExposedByRoute)),
            },
            input_unit: input.input_unit.clone(),
            input_unit_missing_reason: match input.input_unit {
                Some(_) => None,
                None => Some(input.input_unit_missing_reason.unwrap_or(MissingReason::NotExposedByRoute)),
            },
        },
    }
}

/// Session totals by the names 2.1.0 used, with their summary type and unit.
pub const SUMMARY_NAMES: &[(&str, &str, &str)] = &[
    ("duration_s", "sleep_duration", "s"),
    ("time_in_bed_s", "time_in_bed", "s"),
    ("efficiency_pct", "sleep_efficiency", "%"),
    ("deep_s", "deep_sleep", "s"),
    ("rem_s", "rem_sleep", "s"),
    ("light_s", "light_sleep", "s"),
    ("awake_s", "awake", "s"),
    ("interruptions", "sleep_interruptions", "count"),
    ("avg_hr_bpm", "average_heart_rate", "bpm"),
    ("max_hr_bpm", "max_heart_rate", "bpm"),
    ("energy_kcal", "energy", "kcal"),
    ("distance_m", "distance", "m"),
    ("elevation_gain_m", "elevation_gain", "m"),
];

/// One session total a reader found, by its 2.1.0 name, with what the source wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregate {
    pub name: String,
    pub value: f64,
    pub input: Option<SourceInput>,
}

/// Session totals in the order the reader found them. Names outside
/// `SUMMARY_NAMES` are left out, as 2.1.0 left them out.
pub fn source_reported_summaries(aggregates: &[Aggregate]) -> Vec<SourceReportedSummary> {
    aggregates
        .iter()
        .filter_map(|aggregate| {
            let (_, summary_type, unit) = SUMMARY_NAMES.iter().find(|(name, _, _)| *name == aggregate.name)?;
            let value = source_value(aggregate.value, unit, aggregate.input.as_ref());
            Some(SourceReportedSummary {
                standard_value: value.standard_value,
                standard_unit: value.standard_unit,
                input_value: value.input_value,
                input_unit: value.input_unit,
                input_value_missing_reason: value.input_value_missing_reason,
                input_unit_missing_reason: value.input_unit_missing_reason,
                summary_type: summary_type.to_string(),
            })
        })
        .collect()
}

/// The facts the 2.1.0 value-basis rule reads (D187).
#[derive(Debug, Clone, Copy)]
pub struct BasisFacts<'a> {
    /// The source `data_route`, such as `whoop_csv`.
    pub data_route: &'a str,
    pub record_type: RecordType,
    pub quantity: Option<&'a str>,
    /// This program calculated the value during import.
    pub derived: bool,
    /// The record belongs to the manual source.
    pub manual: bool,
}

/// The basis for a value this import stores (D187).
///
/// A value this program calculated says so, even from a hand-entered input.
/// Apple sleep totals are sums of the writer's sleep-stage samples. A value a
/// person typed into Apple Health is `entered_by_hand`. WHOOP and Oura label
/// their HRV column without naming the statistic, and both document it as
/// RMSSD (D22, D26).
pub fn value_basis(facts: BasisFacts<'_>) -> ValueBasis {
    if facts.derived
        || (facts.data_route == "apple_health" && facts.record_type == RecordType::SleepSession)
        || facts.quantity == Some("ecg_beats")
    {
        return ValueBasis::CalculatedOnImport;
    }
    if facts.manual {
        return ValueBasis::EnteredByHand;
    }
    if facts.quantity == Some("hrv_rmssd") && matches!(facts.data_route, "whoop_csv" | "oura_csv") {
        return ValueBasis::ProviderDocumentedRmssd;
    }
    ValueBasis::Provider
}

/// A `field_basis` that names only the value basis.
pub fn value_field_basis(basis: ValueBasis) -> FieldBasis {
    FieldBasis { activity_type: None, value: Some(basis), route_summaries: None }
}

/// True when two record data objects are the same once `received_at` and
/// `received_at_basis` are removed at every depth. A repeated import of an
/// unchanged provider record matches by this rule.
pub fn same_logical_data(left: &Value, right: &Value) -> bool {
    fn strip(value: &Value) -> Value {
        match value {
            Value::Array(items) => Value::Array(items.iter().map(strip).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(key, _)| key.as_str() != "received_at" && key.as_str() != "received_at_basis")
                    .map(|(key, child)| (key.clone(), strip(child)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
    json::to_sorted_string(&strip(left)) == json::to_sorted_string(&strip(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_text_reads_only_an_explicit_offset() {
        assert_eq!(offset_text("2026-08-09T06:05:00-07:00").as_deref(), Some("-07:00"));
        assert_eq!(offset_text("2026-08-09T06:05:00z").as_deref(), Some("Z"));
        assert_eq!(offset_text("2026-08-09T06:05:00-0700"), None);
        assert_eq!(offset_text("2026-08-09 06:05:00"), None);
    }

    #[test]
    fn logical_data_ignores_receipt_times_at_every_depth() {
        let left = json::parse(r#"{"a":1,"received_at":"x","b":{"received_at_basis":"y","c":[1,{"received_at":2}]}}"#)
            .unwrap();
        let right = json::parse(r#"{"b":{"c":[1,{}]},"a":1}"#).unwrap();
        assert!(same_logical_data(&left, &right));
        let changed = json::parse(r#"{"b":{"c":[2,{}]},"a":1}"#).unwrap();
        assert!(!same_logical_data(&left, &changed));
    }

    #[test]
    fn value_basis_follows_the_published_rule() {
        let facts = BasisFacts {
            data_route: "whoop_csv",
            record_type: RecordType::PointMeasurement,
            quantity: Some("hrv_rmssd"),
            derived: false,
            manual: false,
        };
        assert_eq!(value_basis(facts), ValueBasis::ProviderDocumentedRmssd);
        assert_eq!(value_basis(BasisFacts { quantity: Some("hrv_sdnn"), ..facts }), ValueBasis::Provider);
        assert_eq!(value_basis(BasisFacts { manual: true, ..facts }), ValueBasis::EnteredByHand);
        assert_eq!(value_basis(BasisFacts { derived: true, manual: true, ..facts }), ValueBasis::CalculatedOnImport);
    }
}
