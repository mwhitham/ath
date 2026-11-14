//! `ath log` as library operations: the log parts of
//! `tests/v1-cli-writes.test.ts` and `tests/v1-cli-sidecar.test.ts`.

#[path = "scenario_support/mod.rs"]
mod common;

use athleticstandard::json::{self, Value};
use athleticstandard::log::{add_attachments, apply_log_plan, plan_log_entry, LogPlan, LogRecord, LogRequest};
use athleticstandard::model::*;
use athleticstandard::prediction::benchmarks::{seed_benchmark_records, SEED_BENCHMARKS};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{Changes, NewRecord, OpenOptions, Store};
use athleticstandard::Error;
use common::builders::id;
use common::*;
use serde_json::json;

const AT: &str = "2026-10-13T12:00:00.000Z";

/// A database as `ath init -y` leaves it: a profile, the manual source, and
/// the seeded benchmarks.
fn initialized() -> (std::path::PathBuf, Store) {
    let (directory, mut store) = new_store("2026-10-01T00:00:00.000Z");
    let tool = athleticstandard::prediction::grade::tool_actor();
    let mut creates = vec![
        NewRecord::new(
            RecordData::AthleteProfile(AthleteProfileData {
                name: None,
                birth_year: None,
                sex: None,
                display_units: Some(DisplayUnits::Metric),
                default_time_zone: None,
            }),
            tool.clone(),
        ),
        NewRecord::new(
            RecordData::Source(SourceData {
                source_type: SourceType::Manual,
                data_writer: "ath".into(),
                data_route: "manual_entry".into(),
                vendor: None,
                sensor_type: None,
                detail: Some("Manually recorded data".into()),
                supplied_fields: None,
            }),
            tool,
        ),
    ];
    creates.extend(seed_benchmark_records("2026-10-01T00:00:00.000Z"));
    store.apply(Changes { creates, ..Changes::default() }).unwrap();
    (directory, store)
}

fn plan(store: &Store, text: &str) -> athleticstandard::Result<LogPlan> {
    plan_as(store, text, RecordActor::Athlete)
}

fn plan_as(store: &Store, text: &str, actor: RecordActor) -> athleticstandard::Result<LogPlan> {
    let mut new_id = athleticstandard::ids::new_record_id;
    plan_log_entry(
        store.connection(),
        &LogRequest { text: text.into(), actor, created_at: Some(AT.into()), benchmark: None },
        &mut new_id,
    )
}

fn write_log(store: &mut Store, text: &str, actor: RecordActor) {
    let planned = plan_as(store, text, actor).unwrap();
    apply_log_plan(store, &planned).unwrap();
}

fn records(store: &Store, record_type: RecordType) -> Vec<CurrentRecord> {
    store.list_records(&ListOptions { record_type: Some(record_type), include_deleted: true, limit: 10_000 }).unwrap()
}

fn note(text: &str) -> Value {
    json!({
        "report_type": "note",
        "reported_at": AT,
        "input_reported_at": common::builders::receipt(AT),
        "reported_via": "text",
        "note": text,
    })
}

#[test]
fn init_seeds_the_well_known_benchmarks() {
    let (_directory, store) = initialized();
    let definitions = records(&store, RecordType::BenchmarkDefinition);
    let keys: Vec<String> = definitions
        .iter()
        .map(|record| match &record.data {
            RecordData::BenchmarkDefinition(data) => data.benchmark_key.clone(),
            _ => unreachable!(),
        })
        .collect();
    let seeded: Vec<&str> = SEED_BENCHMARKS.iter().map(|seed| seed.key).collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    let mut sorted_seeded = seeded.clone();
    sorted_seeded.sort();
    assert_eq!(sorted_keys, sorted_seeded);
    let author = json::to_value(&definitions[0].data.authorship().unwrap().author);
    assert_eq!(author["calculation_version"], "benchmark-definition-v1");
}

#[test]
fn a_preview_writes_nothing_and_a_write_is_visibly_manual() {
    let (_directory, mut store) = initialized();
    let before = store.database_revision().unwrap();
    let planned = plan(&store, "RMSSD 61 ms").unwrap();
    let preview = json::to_value(&planned.preview("athlete.ath.sqlite"));
    assert_eq!(preview["interpretation"], "manual hrv_rmssd measurement");
    assert_eq!(preview["write_confirmed"], false);
    assert_eq!(store.database_revision().unwrap(), before);
    assert!(records(&store, RecordType::PointMeasurement).is_empty());

    apply_log_plan(&mut store, &planned).unwrap();
    let written = records(&store, RecordType::PointMeasurement);
    let RecordData::PointMeasurement(point) = &written[0].data else { panic!("point") };
    assert_eq!(point.quantity, PointMeasurementType::HrvRmssd);
    assert_eq!((point.value.standard_value, point.value.standard_unit.as_str()), (61.0, "ms"));
    assert_eq!(point.field_basis.as_ref().and_then(|basis| basis.value), Some(ValueBasis::EnteredByHand));
    let manual = records(&store, RecordType::Source)
        .into_iter()
        .find(|record| matches!(&record.data, RecordData::Source(source) if source.source_type == SourceType::Manual))
        .unwrap();
    assert_eq!(point.source_id, manual.record_id);
    // A manual value stays out of device queries and baselines.
    let device = store
        .query_measurements(&athleticstandard::store::query::MeasurementQuery {
            quantity: Some(PointMeasurementType::HrvRmssd),
            ..Default::default()
        })
        .unwrap();
    assert!(device.is_empty());
}

#[test]
fn every_value_entered_through_log_is_marked_entered_by_hand() {
    let (_directory, mut store) = initialized();
    write_log(&mut store, "fran 3:15 rx", RecordActor::Athlete);
    let RecordData::BenchmarkResult(result) = &records(&store, RecordType::BenchmarkResult)[0].data else {
        panic!("result")
    };
    assert_eq!(result.field_basis.as_ref().and_then(|basis| basis.value), Some(ValueBasis::EnteredByHand));
    assert_eq!(result.score.duration_s, Some(195.0));

    let manual = result.source_id.clone();
    let point = |quantity: &str, value: f64, unit: &str, basis: Option<&str>| {
        let mut data = common::builders::point(&id(1), &manual, quantity, AT, value, unit)["data"].clone();
        if let Some(basis) = basis {
            data["field_basis"] = json!({ "value": basis });
        }
        json!({ "record_type": "point_measurement", "data": data })
    };
    let batch = json::to_string(&json!([
        point("body_weight", 70.0, "kg", None),
        point("height", 180.0, "cm", Some("provider"))
    ]));
    write_log(&mut store, &batch, RecordActor::Athlete);
    let bases: Vec<(PointMeasurementType, Option<ValueBasis>)> = records(&store, RecordType::PointMeasurement)
        .iter()
        .map(|record| match &record.data {
            RecordData::PointMeasurement(data) => {
                (data.quantity, data.field_basis.as_ref().and_then(|basis| basis.value))
            }
            _ => unreachable!(),
        })
        .collect();
    assert!(bases.contains(&(PointMeasurementType::BodyWeight, Some(ValueBasis::EnteredByHand))));
    assert!(bases.contains(&(PointMeasurementType::Height, Some(ValueBasis::Provider))));
}

#[test]
fn ambiguous_hrv_is_refused_and_nothing_is_written() {
    let (_directory, store) = initialized();
    let error = plan(&store, "HRV 61 ms").unwrap_err().to_string();
    assert!(error.contains("Say RMSSD or SDNN"), "{error}");
    assert!(records(&store, RecordType::PointMeasurement).is_empty());
}

#[test]
fn a_batch_is_written_as_one_unit() {
    let (_directory, mut store) = initialized();
    let batch = json::to_string(&json!([
        { "record_type": "athlete_report", "data": note("Valid first record") },
        { "record_type": "point_measurement", "data": { "source_id": "not-a-record-id" } },
    ]));
    assert!(plan(&store, &batch).is_err());
    assert!(records(&store, RecordType::AthleteReport).is_empty());

    // A plan can pass and the write still be refused; then nothing is kept.
    let before = store.database_revision().unwrap();
    let negative = plan(&store, "weight -5 kg").unwrap();
    let error = apply_log_plan(&mut store, &negative).unwrap_err();
    assert!(matches!(error, Error::Invalid(_)), "{error}");
    assert_eq!(store.database_revision().unwrap(), before);
    assert!(records(&store, RecordType::PointMeasurement).is_empty());
}

#[test]
fn structured_logs_refuse_unknown_fields_and_check_extensions() {
    let (_directory, mut store) = initialized();
    let unknown = json::to_string(&json!({ "record_type": "athlete_report", "mood_emoji": "ok", "data": note("x") }));
    let error = plan(&store, &unknown).unwrap_err().to_string();
    assert!(error.contains("unknown field") && error.contains("mood_emoji"), "{error}");

    let extensions = json!({ "example.com/mood-app": { "app_mood": "great" } });
    let valid =
        json::to_string(&json!({ "record_type": "athlete_report", "extensions": extensions, "data": note("x") }));
    write_log(&mut store, &valid, RecordActor::Athlete);
    let report = records(&store, RecordType::AthleteReport).remove(0);
    assert_eq!(json::to_string(&Value::Object(report.extensions.unwrap())), json::to_string(&extensions));

    let bad_namespace = json::to_string(&json!({
        "record_type": "athlete_report", "extensions": { "example.com/x": { "app_mood": "great" } }, "data": note("x"),
    }));
    assert!(plan(&store, &bad_namespace).unwrap_err().to_string().contains("namespaced extension"));
    let claims_source = json::to_string(&json!({
        "record_type": "athlete_report",
        "extensions": { "example.com/mood-app": { "source_id": id(0xaa) } },
        "data": note("x"),
    }));
    assert!(plan(&store, &claims_source).unwrap_err().to_string().contains("source_id"));
    assert_eq!(records(&store, RecordType::AthleteReport).len(), 1);
}

#[test]
fn callers_cannot_create_links_or_grades() {
    let (_directory, store) = initialized();
    for record_type in ["prediction_result_link", "prediction_grade"] {
        let text = json::to_string(&json!({ "record_type": record_type, "data": {} }));
        let error = plan(&store, &text).unwrap_err().to_string();
        assert!(error.contains(&format!("structured log cannot create {record_type}; use ath grade")), "{error}");
    }
}

#[test]
fn one_open_prediction_is_linked_and_graded_with_its_result() {
    let (_directory, mut store) = initialized();
    let fran = records(&store, RecordType::BenchmarkDefinition)
        .into_iter()
        .find(|record| matches!(&record.data, RecordData::BenchmarkDefinition(data) if data.benchmark_key == "fran"))
        .unwrap();
    let prediction =
        common::builders::prediction(&id(1), &fran.record_id, "2026-10-12T12:00:00.000Z", 280.0)["data"].clone();
    let agent = RecordActor::Agent { agent: "test-agent".into(), agent_version: None };
    let text = json::to_string(&json!({ "record_type": "prediction", "data": prediction }));
    write_log(&mut store, &text, agent);

    let planned = plan(&store, "Fran in 5:00 rx").unwrap();
    assert_eq!(planned.interpretation, "fran benchmark result; added 1 unambiguous prediction link and grade");
    let written = apply_log_plan(&mut store, &planned).unwrap();
    assert_eq!(written.records.len(), 3);
    assert_eq!(records(&store, RecordType::BenchmarkResult).len(), 1);
    assert_eq!(records(&store, RecordType::PredictionResultLink).len(), 1);
    let grades = records(&store, RecordType::PredictionGrade);
    let RecordData::PredictionGrade(grade) = &grades[0].data else { panic!("grade") };
    assert_eq!(
        (grade.signed_error, grade.absolute_error_percentage, grade.result_in_predicted_range),
        (-20.0, 6.7, false)
    );
}

#[test]
fn a_prediction_error_analysis_is_logged_as_authored_work() {
    let (_directory, mut store) = replay_demo();
    let grade = records(&store, RecordType::PredictionGrade).remove(0);
    let reference = json!({ "record_id": grade.record_id, "revision": grade.current_revision });
    let analysis = json::to_string(&json!({
        "record_type": "prediction_error_analysis",
        "data": {
            "authorship": {
                "author": { "author_type": "agent", "agent": "test-agent", "model": "test-model" },
                "authored_at": AT,
                "evidence": [reference],
                "content_version": 1,
                "status": "active",
            },
            "prediction_grade": reference,
            "error_direction": "slower",
            "error_severity": "minor",
            "causes": [],
            "unexplained": true,
            "lesson": "Collect another result before assigning a cause.",
        },
    }));
    let agent = RecordActor::Agent { agent: "test-agent".into(), agent_version: None };
    write_log(&mut store, &analysis, agent);
    assert_eq!(records(&store, RecordType::PredictionErrorAnalysis).len(), 2);
}

fn attachment(record_id: &str) -> LogRecord {
    LogRecord::attachment(
        record_id.into(),
        AT.into(),
        RecordActor::Athlete,
        AttachmentData {
            file_name: "meal.png".into(),
            media_type: "image/png".into(),
            byte_count: 68,
            sha256: "a".repeat(64),
            description: None,
        },
    )
}

#[test]
fn attachments_link_to_the_one_report_in_the_batch() {
    let (_directory, mut store) = initialized();
    let planned = plan(&store, "Meal photo").unwrap();
    assert_eq!(add_attachments(planned.clone(), Vec::new()).unwrap(), planned);
    let with = add_attachments(planned, vec![attachment(&id(0xa1))]).unwrap();
    assert_eq!(with.interpretation, "athlete note with 1 attachment");
    let preview = json::to_value(&with.preview("file"));
    assert_eq!(preview["records"][0]["record_type"], "attachment");
    assert_eq!(preview["records"][1]["data"]["attachment_ids"], json!([id(0xa1)]));
    assert!(records(&store, RecordType::Attachment).is_empty());
    apply_log_plan(&mut store, &with).unwrap();
    let report = records(&store, RecordType::AthleteReport).remove(0);
    assert_eq!(report.data.attachment_ids(), [id(0xa1)]);
    let linked: i64 = store
        .connection()
        .query_row("SELECT count(*) FROM record_attachment_links WHERE attachment_record_id = ?1", [id(0xa1)], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(linked, 1);

    let measurement = plan(&store, "RMSSD 61 ms").unwrap();
    let error = add_attachments(measurement, vec![attachment(&id(0xa2))]).unwrap_err().to_string();
    assert_eq!(error, "attachments need exactly one athlete_report or nutrition_entry in the log batch");
    let two = json::to_string(&json!([
        { "record_type": "athlete_report", "data": note("one") },
        { "record_type": "athlete_report", "data": note("two") },
    ]));
    assert!(add_attachments(plan(&store, &two).unwrap(), vec![attachment(&id(0xa3)), attachment(&id(0xa4))]).is_err());
}

fn sidecars(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    ["-wal", "-shm"]
        .iter()
        .map(|suffix| athleticstandard::files::sidecar(path, suffix))
        .filter(|candidate| candidate.exists())
        .collect()
}

fn use_delete_journal_mode(path: &std::path::Path) {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mode: String = connection.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0)).unwrap();
    assert_eq!(mode, "delete");
}

#[test]
fn a_read_only_log_write_is_refused_and_leaves_no_wal_files() {
    let (directory, store) = initialized();
    let path = store.path().to_path_buf();
    store.close().unwrap();
    use_delete_journal_mode(&path);
    let mut reader = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let planned = plan(&reader, "felt good").unwrap();
    let error = apply_log_plan(&mut reader, &planned).unwrap_err();
    assert!(matches!(error, Error::Refused(_)), "{error}");
    drop(reader);
    assert!(sidecars(&path).is_empty(), "{:?}", sidecars(&path));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_database_that_needs_migration_refuses_before_a_log_is_planned() {
    let (directory, store) = initialized();
    let path = store.path().to_path_buf();
    store.close().unwrap();
    use_delete_journal_mode(&path);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("UPDATE database_metadata SET sql_schema_version = 4; PRAGMA user_version = 4;").unwrap();
    drop(connection);
    let error = Store::open(&path, OpenOptions::default()).unwrap_err();
    assert!(matches!(error, Error::MigrationRequired(_)), "{error}");
    assert!(sidecars(&path).is_empty(), "{:?}", sidecars(&path));
    std::fs::remove_dir_all(directory).unwrap();
}
