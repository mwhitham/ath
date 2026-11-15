//! Garmin files, Strava exports, and the Garmin pull compared with 2.1.0.
//!
//! The scenarios in `native/tests/golden/routes/{watch,bulk,connection}-*.json`
//! were written by `native/tools/reference/route-golden.ts` from the 2.1.0
//! helper. `route_support::keyed` names the normalizations. The other tests
//! port `tests/v2-core.test.ts`, the Garmin, Strava, and pull rows of
//! `tests/import-baseline.test.ts`, and the helper's own tests.

mod import_support;
mod route_support;

use athleticstandard::import::{self, ImportOptions, SharedImport};
use athleticstandard::json::Value;
use athleticstandard::model::{RecordData, RecordType};
use athleticstandard::runtime::garmin_pull;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::Store;
use import_support::{baseline, new_store, TempDir};
use route_support::exports;
use route_support::keyed::{run_keyed_scenario, step_time, PersonalServer, PERSONAL_KEY};

fn options(received_at: &str) -> ImportOptions {
    ImportOptions { received_at: Some(received_at.to_string()) }
}

fn import_shared(store: &mut Store, path: &std::path::Path, kind: SharedImport, received_at: &str) -> Value {
    let preview = import::preview_shared(store, path, kind, &options(received_at)).unwrap();
    preview.apply(store).unwrap();
    preview.report_value(true).unwrap()
}

fn ids(value: &Value) -> Vec<String> {
    value["record_ids"].as_array().unwrap().iter().map(|id| id.as_str().unwrap().to_string()).collect()
}

fn strings(value: &Value, field: &str) -> Vec<String> {
    value[field].as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect()
}

fn revisions(store: &Store, record_id: &str) -> usize {
    store.revisions(record_id).unwrap().len()
}

#[test]
fn garmin_files_fresh_repeat_and_change_match_2_1_0() {
    run_keyed_scenario("watch-files");
}

#[test]
fn a_nested_garmin_folder_matches_2_1_0() {
    run_keyed_scenario("watch-folder");
}

#[test]
fn single_garmin_files_and_an_archive_match_2_1_0() {
    run_keyed_scenario("watch-containers");
}

#[test]
fn left_out_and_unusual_garmin_files_match_2_1_0() {
    run_keyed_scenario("watch-edges");
}

#[test]
fn garmin_refusals_match_2_1_0() {
    for name in ["watch-refusals", "watch-refusals-2", "watch-refusals-3", "watch-refusals-4", "watch-refusals-5"] {
        run_keyed_scenario(name);
    }
}

#[test]
fn strava_exports_match_2_1_0() {
    for name in ["bulk-folder", "bulk-fixture", "bulk-refusals", "bulk-refusals-2"] {
        run_keyed_scenario(name);
    }
}

#[test]
fn garmin_pulls_match_2_1_0() {
    run_keyed_scenario("connection-day");
}

/// 2.1.0 stopped with `record … was not found` when a later Garmin import
/// named a deleted workout, after writing the records before it. The native
/// import leaves the deleted workout out, writes the rest, and says so.
#[test]
fn a_deleted_garmin_workout_is_left_out() {
    let golden = route_support::golden("watch-deleted.json");
    let message = golden["steps"][1]["write"]["error"].as_str().unwrap();
    assert!(message.starts_with("record <workout_session garmin_account/v1 1001> was not found"), "{message}");

    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let file = exports().join("garmin/park-loop.json");
    let first = import_shared(&mut store, &file, SharedImport::Garmin, &step_time(0));
    let workout = ids(&first)[0].clone();
    store
        .apply(athleticstandard::store::Changes {
            deletions: vec![athleticstandard::store::Deletion {
                record_id: workout.clone(),
                expected_revision: 1,
                deleted_by: athleticstandard::model::RecordActor::Athlete,
                deleted_at: Some(step_time(1)),
            }],
            ..Default::default()
        })
        .unwrap();
    let revision = store.database_revision().unwrap();
    let preview = import::preview_shared(&store, &file, SharedImport::Garmin, &options(&step_time(1))).unwrap();
    let report = preview.report_value(false).unwrap();
    assert_eq!(strings(&report, "left_out"), [format!("1001: the stored record {workout} is deleted")]);
    let result = preview.apply(&mut store).unwrap();
    assert_eq!(result.database_revision, revision, "the unchanged summary writes nothing");
    let written = preview.report_value(true).unwrap();
    assert_eq!(ids(&written), [ids(&first)[1].clone()]);
    assert!(store.get_record(&workout).unwrap().unwrap().is_deleted);
}

/// The import-baseline rows for Garmin, Strava, and the pull.
#[test]
fn shared_fixtures_match_the_retained_import_baseline() {
    let root = import_support::repository_root().join("tests/fixtures/import-baseline");
    let compare = |store: &Store, file: &str| {
        let snapshot = baseline::read_snapshot(store.connection());
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
    };
    for (fixture, kind, file) in [
        ("garmin/park-loop.json", SharedImport::Garmin, "11-park-loop.json"),
        ("garmin/namespaced-hr.gpx", SharedImport::Garmin, "12-namespaced-hr.json"),
        ("garmin/synthetic-position.fit", SharedImport::Garmin, "13-position.json"),
        ("strava", SharedImport::Strava, "14-bulk.json"),
    ] {
        let directory = TempDir::new();
        let mut store = new_store(directory.path());
        import_shared(&mut store, &exports().join(fixture), kind, &step_time(0));
        compare(&store, file);
    }

    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let activities = serde_json::json!([{
        "activityId": 7001,
        "activityName": "Invented personal loop",
        "startTimeGMT": "2026-08-09T15:00:00.000Z",
        "duration": 600,
        "activityType": { "typeKey": "running" },
        "Feeling": "steady"
    }]);
    let hrv = serde_json::json!({ "LastNightAvg": 41, "weeklyAvg": 44, "status": "BALANCED" });
    let server = PersonalServer::start("2026-08-09", activities, Some(hrv));
    let result =
        garmin_pull::pull_day(&mut store, &server.base, PERSONAL_KEY, "2026-08-09", &options(&step_time(0))).unwrap();
    assert!(!athleticstandard::json::to_string(&result).contains(PERSONAL_KEY));
    compare(&store, "15-personal-day.json");
    // LastNightAvg keeps its own summary type.
    let summaries = store
        .list_records(&ListOptions {
            record_type: Some(RecordType::ProviderSummary),
            include_deleted: false,
            limit: 10,
        })
        .unwrap();
    assert!(summaries.iter().any(
        |record| matches!(&record.data, RecordData::ProviderSummary(data) if data.summary_type == "LastNightAvg")
    ));
}

/// `tests/v2-core.test.ts`: Garmin, then Strava, into one database.
#[test]
fn garmin_and_strava_imports_keep_their_meaning() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let garmin = exports().join("garmin/park-loop.json");
    let preview = import::preview_shared(&store, &garmin, SharedImport::Garmin, &options(&step_time(0))).unwrap();
    let previewed = athleticstandard::json::to_value(&preview);
    assert_eq!(previewed["write_confirmed"], Value::Bool(false));
    assert_eq!(previewed["record_ids"], serde_json::json!([]));
    preview.apply(&mut store).unwrap();
    let body = preview.report_value(true).unwrap();
    assert!(strings(&body, "understood").contains(&"name".to_string()));
    assert!(strings(&body, "understood").contains(&"max_heart_rate".to_string()));
    assert!(!strings(&body, "understood").contains(&"maximum_heart_rate_bpm".to_string()));
    assert!(strings(&body, "preserved").contains(&"Feeling".to_string()));
    let record_ids = ids(&body);
    assert_eq!(record_ids.len(), 2);

    let workout = store.get_record(&record_ids[0]).unwrap().unwrap();
    let RecordData::WorkoutSession(data) = &workout.data else { panic!("not a workout") };
    assert_eq!(data.maximum_heart_rate_bpm, None);
    assert_eq!(
        athleticstandard::json::to_value(&data.field_basis),
        serde_json::json!({ "activity_type": "provider_label", "value": "provider" })
    );
    let summary = store.get_record(&record_ids[1]).unwrap().unwrap();
    let RecordData::ProviderSummary(summary) = &summary.data else { panic!("not a summary") };
    assert_eq!(summary.summary_type, "max_heart_rate");
    assert_eq!(summary.value.standard_value, 200.0);
    assert_eq!(summary.value.standard_unit, "bpm");

    // The same file again names the same records and writes no revision.
    let again = import_shared(&mut store, &garmin, SharedImport::Garmin, &step_time(1));
    assert_eq!(ids(&again), record_ids);
    assert_eq!(revisions(&store, &record_ids[0]), 1);

    let strava = import_shared(&mut store, &exports().join("strava"), SharedImport::Strava, &step_time(2));
    assert!(strings(&strava, "preserved").contains(&"Gear".to_string()));
    assert!(strings(&strava, "understood").contains(&"route".to_string()));
    assert_eq!(ids(&strava).len(), 1);
    let ride = store.get_record(&ids(&strava)[0]).unwrap().unwrap();
    let RecordData::WorkoutSession(ride) = &ride.data else { panic!("not a workout") };
    assert_eq!(
        athleticstandard::json::to_value(&ride.field_basis),
        serde_json::json!({ "activity_type": "provider_label", "value": "provider" })
    );
    // Garmin and Strava stay two sources.
    let sources: i64 = store.connection().query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0)).unwrap();
    assert_eq!(sources, 2);
}

/// A changed Garmin activity keeps a stored field the new file leaves out,
/// as the 2.1.0 `revise` merged it, and its revision is a local write.
#[test]
fn a_changed_garmin_activity_is_merged_into_one_revision() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let file = directory.path().join("loop.json");
    let text = std::fs::read_to_string(exports().join("garmin/park-loop.json")).unwrap();
    std::fs::write(&file, &text).unwrap();
    let first = import_shared(&mut store, &file, SharedImport::Garmin, &step_time(0));
    std::fs::write(&file, text.replace("\"description\": \"easy jog\",\n", "").replace("Park Loop", "Park Loop 2"))
        .unwrap();
    import_shared(&mut store, &file, SharedImport::Garmin, &step_time(1));
    let workout = &ids(&first)[0];
    let history = store.revisions(workout).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].became_known_at_basis.as_str(), "provider_import");
    let RecordData::WorkoutSession(data) = &history[1].data else { panic!("not a workout") };
    assert_eq!(data.name.as_deref(), Some("Park Loop 2"));
    assert_eq!(data.description.as_deref(), Some("easy jog"));
    assert_eq!(data.received_at, step_time(1));
}

/// A Strava row names its activity file by the end of its path. A name no
/// file inside the export ends with is never read, even when a file with that
/// name sits beside the export.
#[test]
fn a_strava_activity_file_outside_the_export_is_not_read() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let export = directory.path().join("export");
    std::fs::create_dir_all(export.join("activities")).unwrap();
    let gpx = std::fs::read_to_string(exports().join("strava/activities/river.gpx")).unwrap();
    std::fs::write(directory.path().join("outside.gpx"), &gpx).unwrap();
    std::fs::write(
        export.join("activities.csv"),
        "Activity ID,Activity Date,Activity Name,Activity Type,Elapsed Time,Filename\n1,2026-01-03T16:00:00.000Z,Ride,Ride,2,../outside.gpx\n",
    )
    .unwrap();
    let report = import_shared(&mut store, &export, SharedImport::Strava, &step_time(0));
    assert_eq!(strings(&report, "left_out"), ["../outside.gpx: named in activities.csv and not in the export"]);
    let workout = store.get_record(&ids(&report)[0]).unwrap().unwrap();
    let RecordData::WorkoutSession(data) = &workout.data else { panic!("not a workout") };
    assert!(data.route.is_none());
}

/// A link inside a Garmin or Strava folder is refused rather than followed.
#[cfg(unix)]
#[test]
fn a_symbolic_link_inside_a_shared_folder_is_refused() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let export = directory.path().join("export");
    std::fs::create_dir(&export).unwrap();
    symlink(exports().join("garmin/park-loop.json"), export.join("loop.json")).unwrap();
    let error = import::preview_shared(&store, &export, SharedImport::Garmin, &options(&step_time(0))).unwrap_err();
    assert_eq!(error.to_string(), "refusing symbolic link in provider export: loop.json");
}

/// The pull never prints or stores the key, and a failed token request says
/// what failed without it.
#[test]
fn a_pull_that_cannot_reach_garmin_names_the_request() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error =
        garmin_pull::pull_day(&mut store, &base, PERSONAL_KEY, "2026-08-09", &options(&step_time(0))).unwrap_err();
    let message = error.to_string();
    assert!(message.starts_with("Garmin token request failed: "), "{message}");
    assert!(!message.contains(PERSONAL_KEY));
    let records: i64 = store.connection().query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap();
    assert_eq!(records, 0);
}

/// A token response that is not a JWT stops the pull before any record is read.
#[test]
fn a_token_that_is_not_a_jwt_stops_the_pull() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 4096];
        let _ = stream.read(&mut buffer);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nnoway");
    });
    let error = garmin_pull::fetch_day(&base, PERSONAL_KEY, "2026-08-09").unwrap_err();
    assert_eq!(error.to_string(), "Garmin token response was not a JWT");
}

fn delete(store: &mut Store, record_id: &str, at: &str) {
    let revision = store.get_record(record_id).unwrap().unwrap().current_revision;
    store
        .apply(athleticstandard::store::Changes {
            deletions: vec![athleticstandard::store::Deletion {
                record_id: record_id.to_string(),
                expected_revision: revision,
                deleted_by: athleticstandard::model::RecordActor::Athlete,
                deleted_at: Some(at.to_string()),
            }],
            ..Default::default()
        })
        .unwrap();
}

/// D190 for the file routes: a deleted Garmin source leaves its records out,
/// and no second source is made. 2.1.0 wrote new records under it.
#[test]
fn a_deleted_garmin_source_leaves_its_records_out() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let file = exports().join("garmin/park-loop.json");
    import_shared(&mut store, &file, SharedImport::Garmin, &step_time(0));
    let source: String = store.connection().query_row("SELECT record_id FROM sources", [], |row| row.get(0)).unwrap();
    let records: Vec<String> = store
        .connection()
        .prepare("SELECT record_id FROM records WHERE record_type != 'source'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for id in records {
        delete(&mut store, &id, &step_time(1));
    }
    delete(&mut store, &source, &step_time(1));
    let records_before: i64 =
        store.connection().query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap();
    let other = directory.path().join("other.json");
    std::fs::write(&other, r#"{"activityId":"5","startTimeGMT":"2026-03-01T10:00:00Z"}"#).unwrap();
    let report = import_shared(&mut store, &other, SharedImport::Garmin, &step_time(2));
    assert_eq!(strings(&report, "left_out"), [format!("5: its source {source} is deleted")]);
    assert!(ids(&report).is_empty());
    let records_after: i64 =
        store.connection().query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap();
    assert_eq!(records_after, records_before);
}

/// One key twice in one input: the same data name the record again, and
/// different data refuse the import rather than guess.
#[test]
fn a_repeated_key_in_one_input_repeats_or_refuses() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let same = directory.path().join("same.json");
    let activity = r#"{"activityId":"8","startTimeGMT":"2026-03-01T10:00:00Z","activityName":"Twice"}"#;
    std::fs::write(&same, format!("[{activity},{activity}]")).unwrap();
    let report = import_shared(&mut store, &same, SharedImport::Garmin, &step_time(0));
    let record_ids = ids(&report);
    assert_eq!(record_ids.len(), 2);
    assert_eq!(record_ids[0], record_ids[1]);
    let different = directory.path().join("different.json");
    std::fs::write(&different, format!(r#"[{activity},{{"activityId":"8","startTimeGMT":"2026-03-01T10:00:00Z"}}]"#))
        .unwrap();
    let error = import::preview_shared(&store, &different, SharedImport::Garmin, &options(&step_time(1))).unwrap_err();
    assert_eq!(error.to_string(), "provider export gives one import key two different records: 8");
}

/// A correction from a later import keeps the record's extensions. The 2.1.0
/// helper wrote every revision without them.
#[test]
fn a_changed_activity_keeps_its_extensions() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let file = directory.path().join("loop.json");
    let text = std::fs::read_to_string(exports().join("garmin/park-loop.json")).unwrap();
    std::fs::write(&file, &text).unwrap();
    let first = import_shared(&mut store, &file, SharedImport::Garmin, &step_time(0));
    let workout = ids(&first)[0].clone();
    let current = store.get_record(&workout).unwrap().unwrap();
    let mut extensions = athleticstandard::json::Map::new();
    extensions.insert("org.example/note".into(), serde_json::json!({ "felt": "good" }));
    store
        .apply(athleticstandard::store::Changes {
            corrections: vec![athleticstandard::store::Correction {
                record_id: workout.clone(),
                expected_revision: 1,
                data: current.data.clone(),
                extensions: athleticstandard::store::ExtensionsUpdate::Set(Some(extensions.clone())),
                changed_by: athleticstandard::model::RecordActor::Athlete,
                became_known_at: Some(step_time(1)),
                became_known_at_basis: athleticstandard::model::BecameKnownAtBasis::LocalWrite,
                stale_style: athleticstandard::store::write::StaleStyle::Revision,
            }],
            ..Default::default()
        })
        .unwrap();
    std::fs::write(&file, text.replace("Park Loop", "Park Loop 2")).unwrap();
    import_shared(&mut store, &file, SharedImport::Garmin, &step_time(2));
    let after = store.get_record(&workout).unwrap().unwrap();
    assert_eq!(after.current_revision, 3);
    assert_eq!(after.extensions, Some(extensions));
}

/// A file over the per-file limit is refused before it is read.
#[test]
fn a_shared_file_over_the_limit_is_refused() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let file = directory.path().join("big.gpx");
    std::fs::File::create(&file).unwrap().set_len(athleticstandard::input_policy::MAX_SHARED_FILE_BYTES + 1).unwrap();
    let error = import::preview_shared(&store, &file, SharedImport::Garmin, &options(&step_time(0))).unwrap_err();
    assert_eq!(error.to_string(), "provider file exceeds 268435456 bytes: big.gpx");
}

/// A repeat of a candidate whose stored record is deleted is left out with it.
#[test]
fn a_repeat_of_a_deleted_record_is_left_out_with_it() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let file = directory.path().join("twice.json");
    let activity = r#"{"activityId":"9","startTimeGMT":"2026-03-01T10:00:00Z"}"#;
    std::fs::write(&file, activity).unwrap();
    let first = import_shared(&mut store, &file, SharedImport::Garmin, &step_time(0));
    let workout = ids(&first)[0].clone();
    delete(&mut store, &workout, &step_time(1));
    std::fs::write(&file, format!("[{activity},{activity}]")).unwrap();
    let report = import_shared(&mut store, &file, SharedImport::Garmin, &step_time(2));
    assert!(ids(&report).is_empty());
    assert_eq!(strings(&report, "left_out"), [format!("9: the stored record {workout} is deleted")]);
}
