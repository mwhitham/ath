//! WHOOP import behavior, ported from `tests/import-baseline.test.ts`,
//! `tests/v1-provider-import.test.ts`, and `tests/csv.test.ts`, plus the
//! operation's stale-plan and validation rules.

mod import_support;

use athleticstandard::error::Error;
use athleticstandard::import::candidate::{source_value, MatchKey, Section};
use athleticstandard::import::{
    self, collate, Candidate, CandidateSink, DetectedExport, ImportOptions, ProviderReader,
};
use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{Changes, NewRecord, Store};
use import_support::{baseline, direct, fixture_files, new_store, write_directory, TempDir};

const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";

fn options(received_at: &str) -> ImportOptions {
    ImportOptions { received_at: Some(received_at.to_string()) }
}

fn import_fixture(store: &mut Store, path: &std::path::Path, received_at: &str) -> import::ProviderPreview {
    let preview = import::preview(store, path, &options(received_at)).unwrap();
    assert!(preview.conflicts.is_empty(), "{:?}", preview.conflicts);
    preview.apply(store).unwrap();
    preview
}

fn corrected_copy(directory: &std::path::Path) -> std::path::PathBuf {
    let path = directory.join("corrected");
    let files: Vec<(String, String)> = fixture_files()
        .into_iter()
        .map(|(name, text)| {
            let text = if name == "physiological_cycles.csv" { text.replacen(",68.4,", ",69.4,", 1) } else { text };
            (name, text)
        })
        .collect();
    write_directory(&path, &files);
    path
}

fn import_keys(store: &Store) -> Vec<(String, Value)> {
    let connection = direct(store.path());
    let mut statement = connection
        .prepare("SELECT match_key_method, match_key FROM record_import_keys ORDER BY match_key_method, match_key")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .unwrap()
        .map(|row| {
            let (method, key) = row.unwrap();
            (method, json::parse(&key).unwrap())
        })
        .collect()
}

fn all_records(store: &Store, record_type: RecordType) -> Vec<CurrentRecord> {
    store.list_records(&ListOptions { record_type: Some(record_type), include_deleted: false, limit: 10_000 }).unwrap()
}

fn hrv_record(store: &Store, value: f64) -> CurrentRecord {
    all_records(store, RecordType::PointMeasurement)
        .into_iter()
        .find(|record| match &record.data {
            RecordData::PointMeasurement(data) => {
                data.quantity == PointMeasurementType::HrvRmssd && data.value.standard_value == value
            }
            _ => false,
        })
        .expect("the HRV record")
}

/// SQLite's own checks, plus every current record read back through the validator.
fn assert_database_valid(store: &Store) {
    let integrity: String = direct(store.path()).query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    let foreign: i64 =
        direct(store.path()).query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)).unwrap();
    assert_eq!(foreign, 0);
    let records = store.list_records(&ListOptions { record_type: None, include_deleted: true, limit: 10_000 }).unwrap();
    for record in records {
        assert_eq!(store.revisions(&record.record_id).unwrap().len() as u64, record.current_revision);
    }
}

#[test]
fn the_fixture_matches_the_retained_import_baseline() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_fixture(&mut store, &import_support::whoop_fixture(), RECEIVED_AT);
    let snapshot = baseline::read_snapshot(&direct(store.path()));
    let expected = std::fs::read_to_string(
        import_support::repository_root().join("tests/fixtures/import-baseline/05-cycles.json"),
    )
    .unwrap();
    let text = baseline::comparison_string(&snapshot, Some("field_basis"));
    assert!(!text.contains(&directory.path().display().to_string()));
    assert_eq!(text, expected);
    let basis = std::fs::read_to_string(
        import_support::repository_root().join("tests/fixtures/import-baseline/field-basis/05-cycles.json"),
    )
    .unwrap();
    assert_eq!(baseline::field_basis_string(&snapshot), basis);
    for record in &snapshot.records {
        let value = record.data.get("field_basis").and_then(|basis| basis.get("value"));
        if baseline::MEASURED_RECORD_TYPES.contains(&record.record_type.as_str()) {
            assert!(value.is_some(), "{}", record.record_type);
        } else {
            assert!(record.data.get("field_basis").is_none(), "{}", record.record_type);
        }
    }
}

#[test]
fn imports_matches_a_repeat_and_revisions_a_correction() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &import_support::whoop_fixture(), &options(RECEIVED_AT)).unwrap();
    assert_eq!(preview.provider, "whoop");
    assert_eq!(preview.imported, "WHOOP export");
    assert!(preview.conflicts.is_empty());
    assert!(preview.new_record_count() > 10);
    assert_eq!(preview.matching_record_count(), 0);
    // A preview writes nothing.
    assert_eq!(store.database_revision().unwrap(), 0);
    assert!(all_records(&store, RecordType::Source).is_empty());

    let imported = preview.apply(&mut store).unwrap();
    assert_eq!(imported.new_record_count, preview.new_record_count());
    assert_eq!(imported.database_revision, 1);

    let hrv = hrv_record(&store, 68.4);
    let RecordData::PointMeasurement(data) = &hrv.data else { unreachable!() };
    assert_eq!(data.input_timestamp.timestamp_text.as_deref(), Some("2026-08-09 06:05:00"));
    assert_eq!(data.input_timestamp.utc_offset_text.as_deref(), Some("UTC-07:00"));
    assert_eq!(data.input_timestamp.utc_offset_meaning, Some(UtcOffsetMeaning::ProviderCycleOffset));
    assert_eq!(
        data.input_timestamp.timestamp_basis,
        Some(InputTimestampBasis::SourceLocalTimestampWithDocumentedOffset)
    );
    assert_eq!(data.value.input_value, Some(68.4));
    assert_eq!(data.value.input_unit.as_deref(), Some("ms"));
    let period = &data.provider_reporting_periods.as_ref().unwrap()[0];
    assert_eq!(period.period_type, ProviderReportingPeriodType::WhoopPhysiologicalCycle);
    assert_eq!(period.start_at.as_deref(), Some("2026-08-09T05:10:00.000Z"));
    assert_eq!(period.end_at.as_deref(), Some("2026-08-09T13:20:00.000Z"));
    assert_eq!(period.timezone_offset_text.as_deref(), Some("UTC-07:00"));
    assert_eq!(data.field_basis.as_ref().unwrap().value, Some(ValueBasis::ProviderDocumentedRmssd));
    for record in all_records(&store, RecordType::PointMeasurement) {
        let RecordData::PointMeasurement(data) = &record.data else { unreachable!() };
        assert!(data.recorded_at.as_deref().is_none_or(|time| time.ends_with(".000Z")));
    }
    // Recovery and strain are provider scores, not measurements.
    let scores = all_records(&store, RecordType::ProviderScore);
    let metrics: Vec<(String, f64, String)> = scores
        .iter()
        .map(|record| match &record.data {
            RecordData::ProviderScore(data) => (data.metric.clone(), data.value.standard_value, data.scale.clone()),
            _ => unreachable!(),
        })
        .collect();
    assert!(metrics.contains(&("recovery".into(), 67.0, "0-100".into())));
    assert!(metrics.contains(&("strain".into(), 14.2, "0-21".into())));
    assert!(metrics.contains(&("activity_strain".into(), 11.4, "0-21".into())));

    let repeat =
        import::preview(&store, &import_support::whoop_fixture(), &options("2026-10-14T12:00:00.000Z")).unwrap();
    assert!(repeat.conflicts.is_empty());
    assert_eq!(repeat.new_record_count(), 0);
    assert_eq!(repeat.changed_record_count(), 0);
    assert_eq!(repeat.matching_record_count(), preview.new_record_count());
    // Nothing to write leaves the database revision alone.
    assert_eq!(repeat.apply(&mut store).unwrap().database_revision, 1);

    let corrected = corrected_copy(directory.path());
    let correction = import::preview(&store, &corrected, &options("2026-10-15T12:00:00.000Z")).unwrap();
    assert!(correction.conflicts.is_empty());
    assert_eq!(correction.new_record_count(), 0);
    let changed = correction.changed_records().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].record_id, hrv.record_id);
    assert_eq!(changed[0].current_revision, Some(1));
    correction.apply(&mut store).unwrap();
    let revisions = store.revisions(&hrv.record_id).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].revision, 2);
    assert_eq!(revisions[1].became_known_at, "2026-10-15T12:00:00.000Z");
    assert_eq!(revisions[1].became_known_at_basis, BecameKnownAtBasis::ProviderImport);
    let RecordData::PointMeasurement(data) = &revisions[1].data else { unreachable!() };
    assert_eq!(data.value.standard_value, 69.4);

    let source = &all_records(&store, RecordType::Source)[0].record_id;
    let keys = import_keys(&store);
    let method = "whoop_csv/v1".to_string();
    for expected in [
        serde_json::json!(["source", "WHOOP", "whoop_csv", ""]),
        serde_json::json!(["point_measurement", source, "hrv_rmssd", "2026-08-09T13:05:00.000Z", 0]),
        serde_json::json!(["provider_score", source, "recovery", "2026-08-09T13:05:00.000Z", 0]),
    ] {
        assert!(keys.contains(&(method.clone(), expected.clone())), "{expected}");
    }
    assert_database_valid(&store);
}

#[test]
fn journal_answers_are_athlete_reports_without_a_source() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_fixture(&mut store, &import_support::whoop_fixture(), RECEIVED_AT);
    let reports = all_records(&store, RecordType::AthleteReport);
    let nutrition = all_records(&store, RecordType::NutritionEntry);
    assert_eq!(reports.len() + nutrition.len(), 5);
    assert_eq!(nutrition.len(), 2);
    for record in reports.iter().chain(&nutrition) {
        assert_eq!(record.data.source_id(), None);
    }
    let RecordData::NutritionEntry(entry) = &nutrition[0].data else { unreachable!() };
    assert_eq!(entry.athlete_description, "Have any alcoholic drinks?: yes — two beers");
}

/// Replay a database's records, revisions, and import keys into an empty
/// database with the same IDs, as a package restore leaves them.
fn restore_into(source: &Store, target: &mut Store) {
    let mut changes = Changes::default();
    let records =
        source.list_records(&ListOptions { record_type: None, include_deleted: false, limit: 10_000 }).unwrap();
    for record in records {
        let revision = source.revision(&record.record_id, 1).unwrap().unwrap();
        changes.creates.push(NewRecord {
            data: revision.data,
            extensions: revision.extensions,
            record_id: Some(record.record_id.clone()),
            created_at: Some(record.created_at.clone()),
            became_known_at: Some(revision.became_known_at),
            became_known_at_basis: BecameKnownAtBasis::PackageRestore,
            changed_by: revision.changed_by,
        });
    }
    let connection = direct(source.path());
    let mut statement = connection
        .prepare("SELECT record_id, match_key_method, match_key, imported_at FROM record_import_keys")
        .unwrap();
    changes.import_keys = statement
        .query_map([], |row| {
            Ok(RecordImportKey {
                record_id: row.get(0)?,
                match_key_method: row.get(1)?,
                match_key: row.get(2)?,
                imported_at: row.get(3)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    target.apply(changes).unwrap();
}

#[test]
fn uses_restored_provider_keys_for_a_later_correction() {
    let directory = TempDir::new();
    let mut original = new_store(directory.path());
    import_fixture(&mut original, &import_support::whoop_fixture(), RECEIVED_AT);
    let original_hrv = hrv_record(&original, 68.4);

    let restored_directory = TempDir::new();
    let mut restored = new_store(restored_directory.path());
    restore_into(&original, &mut restored);

    let corrected = corrected_copy(directory.path());
    let correction = import::preview(&restored, &corrected, &options("2026-10-15T12:00:00.000Z")).unwrap();
    assert!(correction.conflicts.is_empty());
    assert_eq!(correction.new_record_count(), 0);
    let changed = correction.changed_records().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].record_id, original_hrv.record_id);
    assert_eq!(changed[0].record_type, "point_measurement");
    assert_eq!(changed[0].current_revision, Some(1));
    correction.apply(&mut restored).unwrap();
    assert_eq!(restored.revisions(&original_hrv.record_id).unwrap().len(), 2);
}

#[test]
fn duplicate_rows_keep_stable_ordinals_and_ambiguous_corrections_are_refused() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let input = directory.path().join("input");
    let duplicate = |text: &str| {
        let lines: Vec<&str> = text.trim_end().split('\n').collect();
        let mut out = vec![lines[0], lines[1], lines[1]];
        out.extend(&lines[2..]);
        out.join("\n") + "\n"
    };
    let files: Vec<(String, String)> = fixture_files()
        .into_iter()
        .map(|(name, text)| if name == "physiological_cycles.csv" { (name, duplicate(&text)) } else { (name, text) })
        .collect();
    write_directory(&input, &files);
    import_fixture(&mut store, &input, RECEIVED_AT);
    let ordinals: Vec<i64> = import_keys(&store)
        .into_iter()
        .filter(|(method, key)| {
            method == "whoop_csv/v1"
                && key[0] == "point_measurement"
                && key[2] == "hrv_rmssd"
                && key[3] == "2026-08-09T13:05:00.000Z"
        })
        .map(|(_, key)| key[4].as_i64().unwrap())
        .collect();
    assert_eq!(ordinals, vec![0, 1]);

    let repeat = import::preview(&store, &input, &options("2026-10-14T12:00:00.000Z")).unwrap();
    assert!(repeat.conflicts.is_empty());
    assert_eq!(repeat.new_record_count() + repeat.changed_record_count(), 0);
    assert!(repeat.matching_record_count() > 0);

    let cycles = input.join("physiological_cycles.csv");
    std::fs::write(&cycles, std::fs::read_to_string(&cycles).unwrap().replacen(",68.4,", ",69.4,", 1)).unwrap();
    let ambiguous = import::preview(&store, &input, &options("2026-10-15T12:00:00.000Z")).unwrap();
    assert_eq!(ambiguous.conflicts.len(), 1);
    assert_eq!(
        ambiguous.conflicts[0].message,
        "an indistinguishable duplicate group changed; the importer will not guess which row was corrected"
    );
    let refused = ambiguous.apply(&mut store).unwrap_err();
    assert!(refused.to_string().contains("unresolved conflicts"), "{refused}");
    assert_eq!(store.database_revision().unwrap(), 1);
}

#[test]
fn a_plan_is_refused_when_the_database_changed_after_the_preview() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &import_support::whoop_fixture(), &options(RECEIVED_AT)).unwrap();
    let mut other = Store::open(store.path(), Default::default()).unwrap();
    other
        .apply(Changes {
            creates: vec![NewRecord::new(
                RecordData::AthleteProfile(AthleteProfileData {
                    name: None,
                    birth_year: None,
                    sex: None,
                    display_units: None,
                    default_time_zone: None,
                }),
                RecordActor::Athlete,
            )],
            ..Changes::default()
        })
        .unwrap();
    let error = preview.apply(&mut store).unwrap_err();
    assert!(matches!(error, Error::Stale(_)), "{error}");
    assert_eq!(error.to_string(), "Database changed after provider import preview; preview it again");
    assert!(all_records(&store, RecordType::Source).is_empty());
}

#[test]
fn a_correction_is_refused_when_its_record_changed_after_the_preview() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_fixture(&mut store, &import_support::whoop_fixture(), RECEIVED_AT);
    let hrv = hrv_record(&store, 68.4);
    let corrected = corrected_copy(directory.path());
    let correction = import::preview(&store, &corrected, &options("2026-10-15T12:00:00.000Z")).unwrap();
    // Another writer corrects the same record and leaves the database revision
    // where the preview saw it, as a direct-SQL writer that skipped the recipe might.
    let RecordData::PointMeasurement(mut data) = hrv.data.clone() else { unreachable!() };
    data.value = source_value(70.0, "ms", None);
    let mut other = Store::open(store.path(), Default::default()).unwrap();
    other
        .apply(Changes {
            corrections: vec![athleticstandard::store::Correction {
                record_id: hrv.record_id.clone(),
                expected_revision: 1,
                data: RecordData::PointMeasurement(data),
                extensions: athleticstandard::store::ExtensionsUpdate::Keep,
                changed_by: RecordActor::Athlete,
                became_known_at: Some("2026-10-14T00:00:00.000Z".into()),
                became_known_at_basis: BecameKnownAtBasis::LocalWrite,
                stale_style: athleticstandard::store::write::StaleStyle::Revision,
            }],
            ..Changes::default()
        })
        .unwrap();
    direct(other.path()).execute("UPDATE database_state SET current_revision = 1", []).unwrap();
    let error = correction.apply(&mut store).unwrap_err();
    assert!(matches!(error, Error::Stale(_)), "{error}");
    assert_eq!(error.to_string(), format!("Record {} changed after preview; preview it again", hrv.record_id));
    assert_eq!(store.revisions(&hrv.record_id).unwrap().len(), 2);
}

#[test]
fn a_plan_is_refused_by_another_database() {
    let first = TempDir::new();
    let second = TempDir::new();
    let store = new_store(first.path());
    let mut other = new_store(second.path());
    let preview = import::preview(&store, &import_support::whoop_fixture(), &options(RECEIVED_AT)).unwrap();
    let error = preview.apply(&mut other).unwrap_err();
    assert!(matches!(error, Error::Stale(_)), "{error}");
}

#[test]
fn skip_reasons_are_listed_in_the_2_1_0_order() {
    let golden = import_support::golden("cycles-edges.json");
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let path = import_support::materialize(&golden["steps"][0]["input"], directory.path());
    let preview = import::preview(&store, &path, &options(RECEIVED_AT)).unwrap();
    let reasons: Vec<&str> = preview.skipped.iter().map(|(reason, _)| reason.as_str()).collect();
    assert_eq!(
        reasons,
        [
            "cycle rows with an unreadable timestamp",
            "journal rows we cannot place in time",
            "sleep rows with an unusable time range",
            "unrecognized WHOOP file: a_b.csv",
            "unrecognized WHOOP file: a-b.csv",
            "unrecognized WHOOP file: a1.csv",
            "unrecognized WHOOP file: extra.csv",
            "workout rows with an unusable time range",
        ]
    );
    let mut sorted = reasons.clone();
    sorted.sort_by(|left, right| collate::locale_compare(left, right));
    assert_eq!(reasons, sorted);
    // The serialized preview keeps that order.
    let text = json::to_pretty(&json::to_value(&preview));
    let positions: Vec<usize> = reasons.iter().map(|reason| text.find(&format!("\"{reason}\"")).unwrap()).collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

/// Deliberate change from 2.1.0: its helper read a cycle timezone made only of
/// letters, such as `PST`, as UTC, which moves every time in the row by hours.
/// The tested 2.1.0 offset rule (`tests/csv.test.ts`) refuses a zone name, so
/// the row is skipped with an example instead.
#[test]
fn a_zone_name_is_skipped_rather_than_read_as_utc() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let input = directory.path().join("input");
    write_directory(
        &input,
        &[(
            "physiological_cycles.csv".into(),
            "Cycle start time,Cycle end time,Cycle timezone,Resting heart rate (bpm)\n\
             2026-08-09 06:00:00,2026-08-09 14:00:00,PST,51\n"
                .into(),
        )],
    );
    let preview = import::preview(&store, &input, &options(RECEIVED_AT)).unwrap();
    assert_eq!(preview.new_record_count(), 1, "only the source");
    assert_eq!(
        preview.skipped,
        vec![(
            "cycle rows with an unreadable timestamp".to_string(),
            import::Skipped { count: 1, example: Some("\"2026-08-09 14:00:00\" with timezone \"PST\"".into()) }
        )]
    );
}

/// An invented route for the operation's own rules. It declares two writers
/// and reports one workout from each, as two watches worn in one session do.
struct TwoWatches {
    invalid: bool,
    undeclared: bool,
}

impl ProviderReader for TwoWatches {
    fn read(&self, _export: &DetectedExport, sink: &mut CandidateSink<'_>) -> athleticstandard::Result<()> {
        for writer in ["Left Watch", "Right Watch"] {
            let source_id = if self.undeclared {
                athleticstandard::ids::new_record_id()
            } else {
                sink.export_source(writer, "invented", None)
            };
            let receipt = import::candidate::offset_timestamp_receipt(
                "2026-08-09T18:00:00-04:00",
                UtcOffsetMeaning::MeaningNotDocumented,
            );
            let data = WorkoutSessionData {
                source_id,
                provider_record_id: None,
                provider_reporting_periods: None,
                received_at: sink.received_at().to_string(),
                received_at_basis: ReceivedAtBasis::ImportTime,
                supplied_fields: None,
                field_basis: None,
                start_at: "2026-08-09T22:00:00.000Z".into(),
                end_at: if self.invalid {
                    "2026-08-09T21:00:00.000Z".into()
                } else {
                    "2026-08-09T23:00:00.000Z".into()
                },
                input_start_timestamp: receipt.clone(),
                input_end_timestamp: receipt,
                activity_type: Some("running".into()),
                name: None,
                description: None,
                text_result: None,
                maximum_heart_rate_bpm: None,
                resting_heart_rate_bpm: None,
                heart_rate_samples: None,
                sets: None,
                route: None,
                source_reported_summaries: Vec::new(),
                segments: None,
            };
            sink.add(Candidate::new(
                RecordData::WorkoutSession(data),
                MatchKey::session(RecordType::WorkoutSession, "2026-08-09T22:00:00.000Z"),
                Section::Record,
            ))?;
        }
        sink.skip_count("rows from an invented file", 2);
        Ok(())
    }
}

fn invented_export(directory: &std::path::Path) -> DetectedExport {
    let path = directory.join("input");
    write_directory(&path, &fixture_files());
    let mut export = import::detect_export(&path).unwrap();
    export.format = import::ExportFormat::Whoop;
    export
}

#[test]
fn two_devices_describing_one_workout_stay_two_records() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let export = invented_export(directory.path());
    let reader = TwoWatches { invalid: false, undeclared: false };
    let preview = import::preview_export(&store, &reader, &export, &options(RECEIVED_AT)).unwrap();
    assert!(preview.conflicts.is_empty());
    assert_eq!(preview.new_record_count(), 4, "two sources and two workouts");
    assert_eq!(preview.skipped[0].1, import::Skipped { count: 2, example: None });
    preview.apply(&mut store).unwrap();
    let workouts = all_records(&store, RecordType::WorkoutSession);
    assert_eq!(workouts.len(), 2);
    assert_ne!(workouts[0].data.source_id(), workouts[1].data.source_id());

    let repeat = import::preview_export(&store, &reader, &export, &options("2026-10-14T12:00:00.000Z")).unwrap();
    assert_eq!(repeat.matching_record_count(), 4);
    assert_eq!(repeat.new_record_count() + repeat.changed_record_count(), 0);
}

#[test]
fn an_invalid_candidate_fails_the_preview_with_its_record_path() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let export = invented_export(directory.path());
    let error = import::preview_export(
        &store,
        &TwoWatches { invalid: true, undeclared: false },
        &export,
        &options(RECEIVED_AT),
    )
    .unwrap_err();
    let Error::Invalid(invalid) = &error else { panic!("{error}") };
    assert_eq!(invalid.issues[0].path_text(), "data.end_at");
    assert_eq!(store.database_revision().unwrap(), 0);
}

#[test]
fn a_measured_candidate_must_name_a_declared_source() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let export = invented_export(directory.path());
    let error = import::preview_export(
        &store,
        &TwoWatches { invalid: false, undeclared: true },
        &export,
        &options(RECEIVED_AT),
    )
    .unwrap_err();
    assert!(error.to_string().contains("did not declare"), "{error}");
}

#[test]
fn received_at_must_be_a_stored_utc_timestamp() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let error =
        import::preview(&store, &import_support::whoop_fixture(), &options("2026-10-13T12:00:00Z")).unwrap_err();
    assert!(matches!(error, Error::Invalid(_)), "{error}");
}

#[test]
fn the_preview_serializes_with_the_2_1_0_field_order() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let preview = import::preview(&store, &import_support::whoop_fixture(), &options(RECEIVED_AT)).unwrap();
    let value = json::to_value(&preview);
    let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "provider",
            "imported",
            "destination_database_revision",
            "new_records",
            "matching_records",
            "changed_records",
            "conflicts",
            "skipped",
            "route_points_stored",
            "write_confirmed",
        ]
    );
    assert_eq!(
        value["new_records"][0],
        serde_json::json!({ "record_id": value["new_records"][0]["record_id"], "record_type": "source" })
    );
    assert_eq!(value["write_confirmed"], Value::Bool(false));
    let planned = preview.planned().unwrap();
    let new_ids: Vec<&str> =
        value["new_records"].as_array().unwrap().iter().map(|item| item["record_id"].as_str().unwrap()).collect();
    let planned_ids: Vec<&str> = planned.iter().map(|record| record.record_id.as_str()).collect();
    assert_eq!(new_ids, planned_ids);
    // The read connection stays usable for other reads after a preview.
    assert!(store.get_record(planned_ids[0]).unwrap().is_none());
}
