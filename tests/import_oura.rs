//! The Oura import compared with 2.1.0.
//!
//! The scenarios in `tests/golden/routes/ring-*.json` were written by
//! `tools/reference/route-golden.ts` from the 2.1.0 library. The other
//! tests port the Oura parts of `tests/import-baseline.test.ts`,
//! `tests/oura-met-column.test.ts`, and `tests/v1-provider-import.test.ts`.

mod import_support;
mod route_support;

use athleticstandard::import::{self, ImportOptions};
use athleticstandard::model::{CurrentRecord, ProviderScoreData, RecordData, RecordType};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::Store;
use import_support::{baseline, new_store, TempDir};
use route_support::{exports, run_provider_scenario};

const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";

fn options(received_at: &str) -> ImportOptions {
    ImportOptions { received_at: Some(received_at.to_string()) }
}

fn import_path(store: &mut Store, path: &std::path::Path) -> import::ProviderPreview {
    let preview = import::preview(store, path, &options(RECEIVED_AT)).unwrap();
    assert!(preview.conflicts.is_empty(), "{:?}", preview.conflicts);
    preview.apply(store).unwrap();
    preview
}

fn records(store: &Store, record_type: RecordType) -> Vec<CurrentRecord> {
    store.list_records(&ListOptions { record_type: Some(record_type), include_deleted: false, limit: 10_000 }).unwrap()
}

fn assert_database_valid(store: &Store) {
    let integrity: String = store.connection().query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    let foreign: i64 =
        store.connection().query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)).unwrap();
    assert_eq!(foreign, 0);
    for record in store.list_records(&ListOptions { record_type: None, include_deleted: true, limit: 10_000 }).unwrap()
    {
        assert_eq!(store.revisions(&record.record_id).unwrap().len() as u64, record.current_revision);
        CurrentRecord::from_value(&record.to_value()).unwrap();
    }
}

fn scores(store: &Store) -> Vec<ProviderScoreData> {
    records(store, RecordType::ProviderScore)
        .into_iter()
        .filter_map(|record| match record.data {
            RecordData::ProviderScore(data) => Some(data),
            _ => None,
        })
        .collect()
}

fn on_day(score: &ProviderScoreData, day: &str) -> bool {
    score
        .provider_reporting_periods
        .as_ref()
        .is_some_and(|periods| periods.iter().any(|period| period.provider_date.as_deref() == Some(day)))
}

#[test]
fn fixture_fresh_repeat_and_correction_match_2_1_0() {
    run_provider_scenario("ring-fixture");
}

#[test]
fn duplicate_rows_and_ambiguous_correction_match_2_1_0() {
    run_provider_scenario("ring-duplicate");
}

#[test]
fn member_portal_portal_spo2_and_offset_tables_match_2_1_0() {
    run_provider_scenario("ring-tables");
}

#[test]
fn zip_and_single_file_containers_match_2_1_0() {
    run_provider_scenario("ring-containers");
}

#[test]
fn edge_rows_match_2_1_0() {
    run_provider_scenario("ring-edges");
}

/// `tests/import-baseline.test.ts` rows 06 to 10.
#[test]
fn every_ring_fixture_matches_the_retained_import_baseline() {
    for (fixture, file) in [
        ("oura", "06-ring.json"),
        ("oura-member-portal", "07-ring-portal.json"),
        ("oura-offset/sleep.csv", "08-ring-offset.json"),
        ("oura-portal", "09-ring-daily.json"),
        ("oura-spo2", "10-ring-spo2.json"),
    ] {
        let directory = TempDir::new();
        let mut store = new_store(directory.path());
        import_path(&mut store, &exports().join(fixture));
        let snapshot = baseline::read_snapshot(store.connection());
        let root = import_support::repository_root().join("tests/fixtures/import-baseline");
        let expected = std::fs::read_to_string(root.join(file)).unwrap();
        assert_eq!(baseline::comparison_string(&snapshot, Some("field_basis")), expected, "{file}");
        let basis = std::fs::read_to_string(root.join("field-basis").join(file)).unwrap();
        assert_eq!(baseline::field_basis_string(&snapshot), basis, "{file}");
        for record in &snapshot.records {
            let value = record.data.get("field_basis").and_then(|basis| basis.get("value"));
            if baseline::MEASURED_RECORD_TYPES.contains(&record.record_type.as_str()) {
                assert!(value.is_some(), "{file}: {} has no value basis", record.record_type);
            } else {
                assert!(record.data.get("field_basis").is_none(), "{file}: {}", record.record_type);
            }
        }
        assert_database_valid(&store);
    }
}

/// `tests/oura-met-column.test.ts`.
#[test]
fn every_ring_fixture_passes_the_database_check() {
    for name in ["oura", "oura-member-portal", "oura-portal", "oura-spo2", "oura-offset"] {
        let directory = TempDir::new();
        let mut store = new_store(directory.path());
        import_path(&mut store, &exports().join(name));
        let result = athleticstandard::store::check::check_database(&store).unwrap();
        assert!(result.valid, "{name}: {:?}", result.issues);
        assert!(result.issues.is_empty(), "{name}: {:?}", result.issues);
    }
}

#[test]
fn a_json_met_cell_counts_as_skipped_activity_data() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let input = directory.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("oura_activity.csv"), "day,met,readiness_score\n2026-01-01,\"{\"\"samples\"\":1}\",70\n")
        .unwrap();
    let preview = import::preview(&store, &input, &options(RECEIVED_AT)).unwrap();
    let skipped: Vec<(&str, u64)> =
        preview.skipped.iter().map(|(reason, skipped)| (reason.as_str(), skipped.count)).collect();
    assert_eq!(skipped, [("oura_activity.csv: activity column met", 1)]);
}

/// `tests/v1-provider-import.test.ts`: date-only scores stay provider days.
#[test]
fn date_only_scores_keep_their_provider_day() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_path(&mut store, &exports().join("oura"));
    let scores = scores(&store);
    let readiness = scores.iter().find(|score| score.metric == "readiness" && on_day(score, "2026-08-09")).unwrap();
    assert_eq!(readiness.recorded_at, None);
    assert_eq!(
        readiness.input_timestamp.timestamp_missing_reason.map(|reason| reason.as_str()),
        Some("not_provided_by_source")
    );
    assert_eq!(readiness.provider_reporting_periods.as_ref().unwrap()[0].period_type.as_str(), "oura_day_unspecified");
    let sleep = scores.iter().find(|score| score.metric == "sleep_score" && on_day(score, "2026-08-09")).unwrap();
    assert_eq!(sleep.recorded_at.as_deref(), Some("2026-08-09T13:08:00.000Z"));
    assert_eq!(sleep.input_timestamp.timestamp_text.as_deref(), Some("2026-08-09T06:08:00-07:00"));
    assert_eq!(sleep.provider_reporting_periods.as_ref().unwrap()[0].period_type.as_str(), "oura_sleep_day");

    let source_id: String = store
        .connection()
        .query_row("SELECT record_id FROM sources WHERE data_writer = 'Oura'", [], |row| row.get(0))
        .unwrap();
    let keys: Vec<String> = store
        .connection()
        .prepare("SELECT match_key FROM record_import_keys WHERE match_key_method = 'oura_csv/v1'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for expected in [
        r#"["source","Oura","oura_csv",""]"#.to_string(),
        format!(r#"["provider_score","{source_id}","readiness","2026-08-09",0]"#),
        format!(r#"["point_measurement","{source_id}","hrv_rmssd","2026-08-09T13:08:00.000Z",0]"#),
    ] {
        assert!(keys.contains(&expected), "{expected}");
    }
}

/// `tests/v1-provider-import.test.ts`: a sleep-day value without a bedtime.
#[test]
fn a_sleep_day_value_without_bedtime_stays_date_only() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let input = directory.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("oura_sleep.csv"), "day,average_hrv\n2026-08-09,63\n").unwrap();
    import_path(&mut store, &input);
    let points = records(&store, RecordType::PointMeasurement);
    assert_eq!(points.len(), 1);
    let RecordData::PointMeasurement(point) = &points[0].data else { panic!("not a point") };
    assert_eq!(point.quantity.as_str(), "hrv_rmssd");
    assert_eq!(point.recorded_at, None);
    let period = &point.provider_reporting_periods.as_ref().unwrap()[0];
    assert_eq!(period.period_type.as_str(), "oura_sleep_day");
    assert_eq!(period.provider_date.as_deref(), Some("2026-08-09"));
    assert_eq!(
        point.field_basis.as_ref().unwrap().value.map(|basis| basis.as_str()),
        Some("provider_documented_rmssd")
    );
}

/// `tests/v1-provider-import.test.ts`: one SpO2 point per row for flat and JSON cells.
#[test]
fn flat_and_json_spo2_cells_each_give_one_point() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import_path(&mut store, &exports().join("oura-spo2"));
    assert_eq!(preview.provider, "oura");
    let oxygen = records(&store, RecordType::PointMeasurement)
        .into_iter()
        .filter(|record| {
            matches!(&record.data, RecordData::PointMeasurement(point) if point.quantity.as_str() == "oxygen_saturation")
        })
        .count();
    assert_eq!(oxygen, 4);
}

/// `tests/v1-provider-import.test.ts`: portal score rows are named by their contributor keys.
#[test]
fn portal_score_rows_are_named_by_contributor_keys() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_path(&mut store, &exports().join("oura-portal"));
    let scores = scores(&store);
    for metric in ["readiness", "sleep_score", "activity_score"] {
        assert_eq!(scores.iter().filter(|score| score.metric == metric).count(), 2, "{metric}");
    }
    for (metric, value) in [("readiness", 74.0), ("sleep_score", 82.0), ("activity_score", 66.0)] {
        assert!(
            scores.iter().any(|score| score.metric == metric && score.value.standard_value == value),
            "{metric} {value}"
        );
    }
}

/// `tests/v1-provider-import.test.ts`: member-portal scores and contributors.
#[test]
fn member_portal_scores_and_contributors_are_stored() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_path(&mut store, &exports().join("oura-member-portal"));
    let scores = scores(&store);
    assert!(scores.iter().filter(|score| score.metric == "readiness").count() >= 2);
    assert!(scores.iter().filter(|score| score.metric == "sleep_score").count() >= 2);
    assert!(scores.iter().filter(|score| score.metric.starts_with("readiness_contributor_")).count() >= 8);
    assert_database_valid(&store);
}

/// A bedtime without an offset is never read in the computer's timezone: the
/// value keeps its day only.
#[test]
fn a_bedtime_without_an_offset_is_not_read_in_local_time() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let input = directory.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(
        input.join("oura_sleep.csv"),
        "day,bedtime_start,bedtime_end,average_hrv\n2026-08-09,2026-08-08T23:00:00,2026-08-09T07:00:00,60\n",
    )
    .unwrap();
    import_path(&mut store, &input);
    assert!(records(&store, RecordType::SleepSession).is_empty());
    let points = records(&store, RecordType::PointMeasurement);
    let RecordData::PointMeasurement(point) = &points[0].data else { panic!("not a point") };
    assert_eq!(point.recorded_at, None);
    assert_eq!(point.provider_reporting_periods.as_ref().unwrap()[0].provider_date.as_deref(), Some("2026-08-09"));
}
