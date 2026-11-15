//! Calculation rules from `tests/v1-calculate.test.ts` and the calculation
//! parts of `tests/v2-core.test.ts`, plus the input and receipt limits.

#[path = "scenario_support/mod.rs"]
mod common;

use athleticstandard::calculate::range::SourceSummary;
use athleticstandard::calculate::{self, CalculateOptions, CalculationResult, Operation};
use athleticstandard::json::{self, Value};
use athleticstandard::store::Store;
use athleticstandard::view::calendar::{resolve_calendar_range, CalendarRangeRequest};
use common::*;
use serde_json::json;

const SOURCE_A: &str = "01890f3e-e7d3-7cc0-88c8-000000000101";
const SOURCE_B: &str = "01890f3e-e7d3-7cc0-88c8-000000000102";
const MANUAL: &str = "01890f3e-e7d3-7cc0-88c8-000000000103";

fn tool() -> Value {
    json!({ "author_type": "tool", "tool": "test", "tool_version": "1.0.0" })
}

fn receipt(at: &str) -> Value {
    json!({
        "timestamp_text": at,
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant",
    })
}

fn source(id: &str, writer: &str, source_type: &str) -> Value {
    json!({
        "record_type": "source",
        "record_id": id,
        "created_at": "2026-10-01T00:00:00.000Z",
        "changed_by": tool(),
        "data": { "source_type": source_type, "data_writer": writer, "data_route": "test" },
    })
}

fn point(id: u32, source: &str, at: &str, value: f64) -> Value {
    json!({
        "record_type": "point_measurement",
        "record_id": format!("01890f3e-e7d3-7cc0-88c8-{id:012x}"),
        "created_at": "2026-10-03T00:00:00.000Z",
        "changed_by": tool(),
        "data": {
            "source_id": source,
            "received_at": "2026-10-03T00:00:00.000Z",
            "received_at_basis": "import_time",
            "quantity": "hrv_rmssd",
            "recorded_at": at,
            "input_timestamp": receipt(at),
            "value": { "standard_value": value, "standard_unit": "ms", "input_value": value, "input_unit": "ms" },
        },
    })
}

/// Two wearable sources and a manual one, as in the 2.1.0 calculation test.
fn calculation_database() -> (std::path::PathBuf, Store) {
    let (directory, mut store) = new_store("2026-10-01T00:00:00.000Z");
    replay(
        &mut store,
        &[json!({ "write": [
            source(SOURCE_A, "Device A", "wearable"),
            source(SOURCE_B, "Device B", "wearable"),
            source(MANUAL, "ath", "manual"),
            point(0x201, SOURCE_A, "2026-10-01T12:00:00.000Z", 60.0),
            point(0x202, SOURCE_A, "2026-10-02T12:00:00.000Z", 70.0),
            point(0x203, SOURCE_B, "2026-10-01T12:00:00.000Z", 50.0),
            point(0x204, SOURCE_B, "2026-10-02T12:00:00.000Z", 55.0),
            point(0x205, MANUAL, "2026-10-01T12:00:00.000Z", 999.0),
        ]})],
    );
    (directory, store)
}

#[test]
fn source_baselines_stay_separate_and_manual_values_stay_out() {
    let (_directory, store) = calculation_database();
    let receipt = calculate::calculate_range(&store, &CalculateOptions::new(Operation::Baseline, "hrv_rmssd")).unwrap();
    assert_eq!(receipt.sources, vec![SOURCE_A.to_string(), SOURCE_B.to_string()]);
    assert_eq!(receipt.input.record_count, 4);
    assert_eq!(
        json::to_string(&json::to_value(&receipt.result)),
        json::to_string(&json!([
            { "source_id": SOURCE_A, "count": 2, "minimum": 60, "maximum": 70, "mean": 65, "sum": 130, "standard_deviation": 5 },
            { "source_id": SOURCE_B, "count": 2, "minimum": 50, "maximum": 55, "mean": 52.5, "sum": 105, "standard_deviation": 2.5 },
        ]))
    );
}

#[test]
fn calendar_coverage_needs_a_timezone() {
    let (_directory, store) = calculation_database();
    let mut options = CalculateOptions::new(Operation::Coverage, "hrv_rmssd");
    options.source_id = Some(SOURCE_A.into());
    let error = calculate::calculate_range(&store, &options).unwrap_err().to_string();
    assert!(error.contains("timezone"), "{error}");
    options.time_zone = Some("America/Chicago".into());
    options.from = Some("2026-10-01".into());
    options.to = Some("2026-10-03".into());
    let receipt = calculate::calculate_range(&store, &options).unwrap();
    let CalculationResult::Coverage(rows) = receipt.result else { panic!("coverage") };
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].days_expected, rows[0].days_present), (3, 2));
    assert_eq!(rows[0].missing_dates, vec!["2026-10-03".to_string()]);
}

#[test]
fn named_sources_are_compared_without_pooling() {
    let (_directory, store) = calculation_database();
    let mut options = CalculateOptions::new(Operation::CompareSources, "hrv_rmssd");
    options.source_id = Some(SOURCE_A.into());
    options.compare_source_id = Some(SOURCE_B.into());
    let receipt = calculate::calculate_range(&store, &options).unwrap();
    let CalculationResult::CompareSources(result) = receipt.result else { panic!("compare") };
    assert_eq!((result.source.summary.mean, result.comparison_source.summary.mean), (65.0, 52.5));
    assert_eq!(result.mean_difference, 12.5);
}

/// Insert many heart-rate samples with direct SQL. The calculation reads
/// typed rows only, so the stored JSON can be a placeholder.
fn insert_samples(store: &mut Store, prefix: &str, count: usize, at: &str, value: f64) {
    write_directly(store.path(), |tx| {
        tx.execute_batch(&format!(
            "CREATE TEMP TABLE IF NOT EXISTS sequence_numbers (n INTEGER PRIMARY KEY) STRICT;
                 DELETE FROM sequence_numbers;
                 WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence WHERE n < {count})
                 INSERT INTO sequence_numbers (n) SELECT n FROM sequence;
                 INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
                   SELECT printf('{prefix}-%06d', n), 'measurement_sample', 1, '{at}', 0 FROM sequence_numbers;
                 INSERT INTO record_revisions (record_id, revision, record_type, record_json, became_known_at,
                     became_known_at_basis, changed_by_json)
                   SELECT printf('{prefix}-%06d', n), 1, 'measurement_sample', '{{}}', '{at}', 'provider_import',
                     '{{\"author_type\":\"tool\",\"tool\":\"test\",\"tool_version\":\"1.0.0\"}}' FROM sequence_numbers;
                 INSERT INTO measurement_samples (record_id, record_type, source_id, quantity, recorded_at,
                     standard_value, standard_unit, received_at, data_json)
                   SELECT printf('{prefix}-%06d', n), 'measurement_sample', '{SOURCE_A}', 'heart_rate', '{at}',
                     {value} + (n % 2), 'bpm', '{at}', '{{}}' FROM sequence_numbers;"
        ))
    });
}

fn workout(id: &str, start: &str, end: &str) -> Value {
    json!({
        "record_type": "workout_session",
        "record_id": id,
        "created_at": start,
        "changed_by": tool(),
        "data": {
            "source_id": SOURCE_A,
            "received_at": end,
            "received_at_basis": "import_time",
            "start_at": start,
            "end_at": end,
            "input_start_timestamp": receipt(start),
            "input_end_timestamp": receipt(end),
            "activity_type": "running",
            "source_reported_summaries": [],
            "segments": [],
        },
    })
}

fn heart_rate(id: u32, at: &str, value: f64) -> Value {
    json!({
        "record_type": "measurement_sample",
        "record_id": format!("01890f3e-e7d3-7cc0-88c8-{id:012x}"),
        "created_at": at,
        "changed_by": tool(),
        "data": {
            "source_id": SOURCE_A,
            "received_at": at,
            "received_at_basis": "import_time",
            "quantity": "heart_rate",
            "recorded_at": at,
            "input_timestamp": receipt(at),
            "value": { "standard_value": value, "standard_unit": "bpm", "input_value": value, "input_unit": "bpm" },
        },
    })
}

#[test]
fn workout_drift_bounds_its_inputs_before_the_safety_limit() {
    let (_directory, mut store) = calculation_database();
    let workout_id = "01890f3e-e7d3-7cc0-88c8-000000000301";
    replay(
        &mut store,
        &[json!({ "write": [
            workout(workout_id, "2026-10-05T12:00:00.000Z", "2026-10-05T12:20:00.000Z"),
            heart_rate(0x302, "2026-10-05T12:05:00.000Z", 120.0),
            heart_rate(0x303, "2026-10-05T12:15:00.000Z", 140.0),
        ]})],
    );
    insert_samples(&mut store, "outside", 100_001, "2026-10-06T12:00:00.000Z", 100.0);
    let mut options = CalculateOptions::new(Operation::WorkoutDrift, "heart_rate");
    options.source_id = Some(SOURCE_A.into());
    options.workout_session_id = Some(workout_id.into());
    let receipt = calculate::calculate_range(&store, &options).unwrap();
    assert_eq!(receipt.input.record_count, 2);
    let result = json::to_value(&receipt.result);
    assert_eq!(result["workout_session_id"], workout_id);
    assert_eq!(result["sources"][0]["available"], true);
    assert_eq!(result["sources"][0]["first_half_mean"], 120.0);
    assert_eq!(result["sources"][0]["second_half_mean"], 140.0);
}

#[test]
fn more_than_100000_inputs_is_refused_until_the_range_is_narrowed() {
    let (_directory, mut store) = calculation_database();
    insert_samples(&mut store, "early", 100_001, "2026-10-06T12:00:00.000Z", 100.0);
    insert_samples(&mut store, "late", 10, "2026-10-08T12:00:00.000Z", 90.0);
    let mut options = CalculateOptions::new(Operation::Summary, "heart_rate");
    let error = calculate::calculate_range(&store, &options).unwrap_err().to_string();
    assert_eq!(error, "Calculation matched more than 100000 records. Narrow --from and --to.");
    options.from = Some("2026-10-07T00:00:00.000Z".into());
    let receipt = calculate::calculate_range(&store, &options).unwrap();
    assert_eq!(receipt.input.record_count, 10);
}

#[test]
fn a_receipt_lists_1000_references_says_so_and_uses_every_input() {
    let (_directory, mut store) = calculation_database();
    insert_samples(&mut store, "many", 1_001, "2026-10-06T12:00:00.000Z", 100.0);
    let receipt = calculate::calculate_range(&store, &CalculateOptions::new(Operation::Summary, "heart_rate")).unwrap();
    assert_eq!(receipt.input.record_count, 1_001);
    assert_eq!(receipt.input.record_references.len(), 1_000);
    assert!(receipt.input.references_truncated);
    assert_eq!(json::to_value(&receipt)["input"]["references_truncated"], true);
    let CalculationResult::Summaries(rows) = receipt.result else { panic!("summary") };
    let SourceSummary::Whole { summary, .. } = &rows[0] else { panic!("whole") };
    // 501 odd numbers give 101 and 500 even numbers give 100.
    assert_eq!((summary.count, summary.sum), (1_001, 100_601.0));
    let small = calculate::calculate_range(&store, &{
        let mut options = CalculateOptions::new(Operation::Summary, "hrv_rmssd");
        options.source_id = Some(SOURCE_A.into());
        options
    })
    .unwrap();
    assert!(!small.input.references_truncated);
}

#[test]
fn calendar_days_use_the_real_short_and_long_day_boundaries() {
    let (_directory, store) = calculation_database();
    let range = |day: &str| {
        resolve_calendar_range(
            &store,
            &CalendarRangeRequest {
                from: Some(day.into()),
                to: Some(day.into()),
                time_zone: Some("America/Chicago".into()),
                raw: false,
            },
        )
        .unwrap()
    };
    let spring = range("2026-03-08");
    assert_eq!(spring.from.as_deref(), Some("2026-03-08T06:00:00.000Z"));
    assert_eq!(spring.to.as_deref(), Some("2026-03-09T04:59:59.999Z"));
    let autumn = range("2026-11-01");
    assert_eq!(autumn.from.as_deref(), Some("2026-11-01T05:00:00.000Z"));
    assert_eq!(autumn.to.as_deref(), Some("2026-11-02T05:59:59.999Z"));
    assert_eq!(autumn.time_zone.as_deref(), Some("America/Chicago"));
}

#[test]
fn an_edited_calculation_file_changes_the_result_and_not_the_shipped_copy() {
    let (_directory, mut store) = new_store("2026-10-01T00:00:00.000Z");
    let workout_id = "01890f3e-e7d3-7cc0-88c8-000000000401";
    let mut record = workout(workout_id, "2026-10-01T12:00:00.000Z", "2026-10-01T12:00:03.000Z");
    record["data"]["heart_rate_samples"] = json!([
        { "offset_ms": 0, "bpm": 100 }, { "offset_ms": 1000, "bpm": 130 },
        { "offset_ms": 2000, "bpm": 160 }, { "offset_ms": 3000, "bpm": 190 },
    ]);
    replay(&mut store, &[json!({ "write": [source(SOURCE_A, "Device A", "wearable"), record.clone()] })]);
    let working = temporary_dir("ath-zones-");
    let missing = calculate::apply(&store, "heart-rate-zones", workout_id, &working).unwrap();
    assert!(missing.missing.contains(&"maximum_heart_rate_bpm".to_string()));

    record["data"]["maximum_heart_rate_bpm"] = json!(200);
    replay(
        &mut store,
        &[json!({ "correct": {
            "record_id": workout_id, "expected_revision": 1, "changed_by": tool(),
            "became_known_at": "2026-10-02T00:00:00.000Z", "data": record["data"],
        }})],
    );
    let zones = calculate::apply(&store, "heart-rate-zones", workout_id, &working).unwrap();
    assert!(zones.missing.is_empty());
    assert_eq!(json::encode(&zones.result), r#"{"seconds":[1,1,0,1,1]}"#);

    let shipped = athleticstandard::assets::shipped_calculation("heart-rate-zones").unwrap();
    let folder = working.join(".agents/skills/heart-rate-zones");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("SKILL.md"), shipped.replace("zone: 0.60", "zone: 0.50")).unwrap();
    let edited = calculate::apply(&store, "heart-rate-zones", workout_id, &working).unwrap();
    assert_ne!(json::encode(&edited.result), r#"{"seconds":[1,1,0,1,1]}"#);
    assert!(shipped.contains("zone: 0.60"));
    std::fs::remove_dir_all(&working).unwrap();
}

/// Every range calculation and a one-record calculation run on a writable
/// connection. Reading a calculation never writes: the database revision and
/// the file's bytes stay the same.
#[test]
fn calculations_write_nothing() {
    let (_directory, mut store) = calculation_database();
    let workout_id = "01890f3e-e7d3-7cc0-88c8-000000000501";
    let mut record = workout(workout_id, "2026-10-02T12:00:00.000Z", "2026-10-02T12:20:00.000Z");
    record["data"]["maximum_heart_rate_bpm"] = json!(200);
    record["data"]["heart_rate_samples"] = json!([{ "offset_ms": 0, "bpm": 120 }, { "offset_ms": 1000, "bpm": 150 }]);
    let score = json!({
        "record_type": "provider_score",
        "record_id": "01890f3e-e7d3-7cc0-88c8-000000000504",
        "created_at": "2026-10-02T00:00:00.000Z",
        "changed_by": tool(),
        "data": {
            "source_id": SOURCE_A,
            "received_at": "2026-10-02T00:00:00.000Z",
            "received_at_basis": "import_time",
            "metric": "recovery",
            "value": { "standard_value": 78, "standard_unit": "score", "input_value": 78, "input_unit": "score" },
            "scale": "0-100",
            "input_timestamp": {
                "timestamp_missing_reason": "not_provided_by_source",
                "utc_offset_missing_reason": "not_provided_by_source",
            },
            "provider_reporting_periods": [
                { "period_type": "whoop_physiological_cycle", "provider_period_id": "cycle-2026-10-02" },
            ],
        },
    });
    replay(
        &mut store,
        &[json!({ "write": [
            record,
            heart_rate(0x502, "2026-10-02T12:05:00.000Z", 120.0),
            heart_rate(0x503, "2026-10-02T12:15:00.000Z", 140.0),
            score,
        ]})],
    );
    let path = store.path().to_path_buf();
    let revision = store.database_revision().unwrap();
    store.close().unwrap();
    let before = std::fs::read(&path).unwrap();

    let store = Store::open(&path, athleticstandard::store::OpenOptions::default()).unwrap();
    for operation in [
        Operation::Summary,
        Operation::Baseline,
        Operation::Trend,
        Operation::Coverage,
        Operation::CompareSources,
        Operation::Correlation,
        Operation::WorkoutDrift,
        Operation::ProviderPeriodSummary,
    ] {
        let mut options = CalculateOptions::new(operation, "hrv_rmssd");
        options.source_id = Some(SOURCE_A.into());
        match operation {
            Operation::Coverage => {
                options.time_zone = Some("UTC".into());
                options.from = Some("2026-10-01".into());
                options.to = Some("2026-10-02".into());
            }
            Operation::Correlation => {
                options.time_zone = Some("UTC".into());
                options.other_quantity = Some("heart_rate".into());
            }
            Operation::CompareSources => options.compare_source_id = Some(SOURCE_B.into()),
            Operation::WorkoutDrift => {
                options.quantity = "heart_rate".into();
                options.workout_session_id = Some(workout_id.into());
            }
            Operation::ProviderPeriodSummary => {
                options.quantity = "recovery".into();
                options.record_kind = calculate::RecordKind::ProviderScore;
                options.provider_period_type =
                    Some(athleticstandard::model::ProviderReportingPeriodType::WhoopPhysiologicalCycle);
            }
            _ => {}
        }
        let receipt = calculate::calculate_range(&store, &options);
        let count = receipt.as_ref().map(|receipt| receipt.input.record_count);
        assert!(count.as_ref().is_ok_and(|count| *count > 0), "{operation:?}: {count:?}");
        assert_eq!(store.database_revision().unwrap(), revision, "{operation:?}");
    }
    let working = temporary_dir("ath-apply-");
    let zones = calculate::apply(&store, "heart-rate-zones", workout_id, &working).unwrap();
    assert!(zones.missing.is_empty(), "{:?}", zones.missing);
    assert_eq!(store.database_revision().unwrap(), revision);
    store.close().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(&working).unwrap();
}
