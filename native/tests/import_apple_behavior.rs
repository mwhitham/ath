//! Apple Health import behavior, ported from `tests/apple.test.ts`,
//! `apple-zip.test.ts`, `apple-route.test.ts`, `apple-route-scope.test.ts`,
//! `apple-rust-parity.test.ts`, `ecg.test.ts`, `routes.test.ts`, and the Apple
//! parts of `import-baseline.test.ts` and `v1-provider-import.test.ts`. The
//! numeric vectors were written by 2.1.0 and are compared exactly.

mod import_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use athleticstandard::import::apple::{self, hrv, routes, rpeaks};
use athleticstandard::import::{self, instant, ImportOptions};
use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{check, Store};
use import_support::{baseline, golden, new_store, repository_root, write_zip, TempDir};

const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";

fn options(received_at: &str) -> ImportOptions {
    ImportOptions { received_at: Some(received_at.to_string()) }
}

fn exports(name: &str) -> PathBuf {
    repository_root().join("tests/fixtures/exports").join(name)
}

fn import_path(store: &mut Store, path: &Path) -> import::ProviderPreview {
    let preview = import::preview(store, path, &options(RECEIVED_AT)).unwrap();
    assert!(preview.conflicts.is_empty(), "{:?}", preview.conflicts);
    preview.apply(store).unwrap();
    preview
}

fn records(store: &Store, record_type: RecordType) -> Vec<Value> {
    store
        .list_records(&ListOptions { record_type: Some(record_type), include_deleted: false, limit: 10_000 })
        .unwrap()
        .into_iter()
        .map(|record| {
            let mut value = record.data.to_value();
            value["record_id"] = Value::String(record.record_id.clone());
            value
        })
        .collect()
}

fn skipped(preview: &import::ProviderPreview, reason: &str) -> Option<(u64, Option<String>)> {
    preview.skipped.iter().find(|(known, _)| known == reason).map(|(_, skip)| (skip.count, skip.example.clone()))
}

fn write_files(root: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn workout_xml(references: &[&str]) -> String {
    let routes: String = references
        .iter()
        .map(|reference| format!("  <WorkoutRoute sourceName=\"Demo Phone\">\n   <FileReference path=\"{reference}\"/>\n  </WorkoutRoute>\n"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<HealthData>\n <Workout workoutActivityType=\"HKWorkoutActivityTypeRunning\" duration=\"40\" durationUnit=\"min\" sourceName=\"Apple Watch\" startDate=\"2026-06-14 08:00:00 -0500\" endDate=\"2026-06-14 08:40:00 -0500\">\n{routes} </Workout>\n</HealthData>\n"
    )
}

fn gpx_at(lat: &str, lon: &str, first: &str, second: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\"><trk><trkseg>\n <trkpt lat=\"{lat}\" lon=\"{lon}\"><time>{first}</time></trkpt>\n <trkpt lat=\"{lat}\" lon=\"{lon}\"><time>{second}</time></trkpt>\n</trkseg></trk></gpx>\n"
    )
}

fn route_positions(workout: &Value) -> Vec<(f64, f64)> {
    workout["route"]
        .as_array()
        .map(|route| {
            route
                .iter()
                .map(|point| (point["latitude_deg"].as_f64().unwrap(), point["longitude_deg"].as_f64().unwrap()))
                .collect()
        })
        .unwrap_or_default()
}

// ---- The import baseline ------------------------------------------------------------

fn baseline_text(file: &str) -> (String, String) {
    let root = repository_root().join("tests/fixtures/import-baseline");
    (
        std::fs::read_to_string(root.join(file)).unwrap(),
        std::fs::read_to_string(root.join("field-basis").join(file)).unwrap(),
    )
}

fn assert_baseline(store: &Store, file: &str, directory: &Path) {
    let snapshot = baseline::read_snapshot(store.connection());
    let text = baseline::comparison_string(&snapshot, Some("field_basis"));
    assert!(!text.contains(&directory.display().to_string()));
    let (expected, basis) = baseline_text(file);
    assert_eq!(text, expected, "{file}");
    assert_eq!(baseline::field_basis_string(&snapshot), basis, "{file}");
    for record in &snapshot.records {
        let value = record.data.get("field_basis").and_then(|basis| basis.get("value"));
        if baseline::MEASURED_RECORD_TYPES.contains(&record.record_type.as_str()) {
            assert!(value.is_some(), "{}", record.record_type);
        } else {
            assert!(record.data.get("field_basis").is_none(), "{}", record.record_type);
        }
    }
}

fn copy_fixture(from: &Path, to: &Path) {
    for name in athleticstandard::import::input::walk_files(from).unwrap() {
        let target = to.join(&name);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(from.join(&name), target).unwrap();
    }
}

#[test]
fn the_fixture_folder_zip_and_bare_xml_match_the_retained_baseline() {
    for (file, path) in [
        ("01-apple-dir.json", exports("apple")),
        ("02-apple-inline.json", exports("apple-inline-route")),
        ("03-apple-file.json", exports("apple-file-reference")),
        ("18-apple-bare-xml.json", exports("apple/export.xml")),
    ] {
        let directory = TempDir::new();
        let mut store = new_store(directory.path());
        import_path(&mut store, &path);
        assert_baseline(&store, file, directory.path());
    }
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let files: Vec<(String, String)> = athleticstandard::import::input::walk_files(&exports("apple"))
        .unwrap()
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(exports("apple").join(&name)).unwrap();
            (format!("apple_health_export/{name}"), text)
        })
        .collect();
    let zip = directory.path().join("health.zip");
    write_zip(&zip, &files);
    import_path(&mut store, &zip);
    assert_baseline(&store, "17-apple-zip.json", directory.path());
}

/// The retained baseline for the route-mismatch fixture read one GPX time
/// without an offset in UTC. 2.1.1 leaves that point out of the route
/// summary (a listed deliberate change), so only that workout's splits differ.
#[test]
fn the_route_mismatch_fixture_differs_from_the_baseline_only_in_the_offset_free_splits() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    import_path(&mut store, &exports("apple-route-mismatch"));
    let snapshot = baseline::read_snapshot(store.connection());
    let (expected, basis) = baseline_text("04-apple-mismatch.json");
    assert_eq!(baseline::field_basis_string(&snapshot), basis);
    let mut actual = json::parse(&baseline::comparison_string(&snapshot, Some("field_basis"))).unwrap();
    let mut expected = json::parse(&expected).unwrap();
    let mut removed = Vec::new();
    for document in [&mut actual, &mut expected] {
        for record in document["records"].as_array_mut().unwrap() {
            if record["data"]["activity_type"] == "cycling" {
                removed.push(record["data"].as_object_mut().unwrap().remove("segments").unwrap());
            }
        }
    }
    assert_eq!(json::to_deterministic_pretty(&actual), json::to_deterministic_pretty(&expected));
    // 2.1.0 read 08:03 as UTC and opened the first split five hours early.
    assert_eq!(removed[1][0]["source_reported_summaries"][0]["standard_value"], 17881.6);
    assert_eq!(removed[0][0]["source_reported_summaries"][0]["standard_value"], 1.6);
    assert_eq!(removed[0].as_array().unwrap().len(), removed[1].as_array().unwrap().len());
}

// ---- v1 provider import ---------------------------------------------------------------

#[test]
fn every_approved_fixture_passes_the_database_check() {
    for name in ["apple", "apple-inline-route", "apple-file-reference", "apple-route-mismatch"] {
        let directory = TempDir::new();
        let mut store = new_store(directory.path());
        import_path(&mut store, &exports(name));
        let result = check::check_database(&store).unwrap();
        assert!(result.valid, "{name}: {:?}", result.issues);
        assert!(result.issues.is_empty(), "{name}: {:?}", result.issues);
    }
}

#[test]
fn dense_samples_become_stable_records_with_their_receipts() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &exports("apple"), &options(RECEIVED_AT)).unwrap();
    assert_eq!(preview.provider, "apple");
    assert_eq!(preview.imported, "Apple Health export");
    assert!(preview.new_records().unwrap().iter().any(|record| record.record_type == "measurement_sample"));
    preview.apply(&mut store).unwrap();

    let samples = records(&store, RecordType::MeasurementSample);
    assert!(samples.iter().any(|sample| sample["input_timestamp"]["timestamp_text"] == "2026-08-09 18:30:00 -0700"
        && sample["input_timestamp"]["utc_offset_text"] == "-0700"
        && sample["value"]["input_value"] == 9.8
        && sample["value"]["input_unit"] == "kcal"
        && sample.get("timestamp_reconstruction_method").is_none()));
    assert!(samples
        .iter()
        .any(|sample| sample["input_timestamp"]["timestamp_basis"] == "reconstructed_from_series_start"
            && sample["timestamp_reconstruction_method"] == "series_start_plus_offset_ms_v1"));
    let heart_rate = samples
        .iter()
        .find(|sample| sample["quantity"] == "heart_rate" && sample["value"]["standard_value"] == 62.0)
        .unwrap();
    assert_eq!(heart_rate["input_timestamp"]["source_time_zone"], "America/Los_Angeles");
    assert_eq!(heart_rate["input_timestamp"]["source_time_zone_meaning"], "apple_health_object_creation_time_zone");

    let measurements = records(&store, RecordType::PointMeasurement);
    let weight = measurements.iter().find(|record| record["quantity"] == "body_weight").unwrap();
    assert_eq!(weight["input_timestamp"]["timestamp_text"], "2026-08-09 07:02:00 -0700");
    assert_eq!(weight["value"]["input_value"], 181.4);
    assert_eq!(weight["value"]["input_unit"], "lb");
    assert_eq!(weight["value"]["standard_unit"], "kg");
    let resting = measurements
        .iter()
        .find(|record| record["quantity"] == "resting_heart_rate" && record["value"]["standard_value"] == 51.0)
        .unwrap();
    assert_eq!(resting["input_timestamp"]["source_time_zone"], "America/Los_Angeles");
    // SDNN stays a sample. RMSSD is calculated from the beats and never equals the SDNN value.
    assert!(measurements.iter().all(|record| record["quantity"] != "hrv_sdnn"));
    let rmssd: Vec<&Value> = measurements.iter().filter(|record| record["quantity"] == "hrv_rmssd").collect();
    assert_eq!(rmssd.len(), 2);
    assert!(rmssd
        .iter()
        .all(|record| record["value"]["standard_value"] != 52.3
            && record["field_basis"]["value"] == "calculated_on_import"));

    let summaries = records(&store, RecordType::ProviderSummary);
    assert_eq!(summaries.len(), 6);
    let energy = summaries.iter().find(|record| record["summary_type"] == "active_energy_burned").unwrap();
    assert_eq!(energy["value"]["standard_value"], 612.4);
    assert_eq!(energy["value"]["input_unit"], "kcal");
    assert_eq!(energy["input_timestamp"]["timestamp_missing_reason"], "not_provided_by_source");
    assert_eq!(energy["provider_reporting_periods"][0]["period_type"], "apple_activity_summary_day");
    assert_eq!(energy["provider_reporting_periods"][0]["provider_date"], "2026-08-09");
    assert!(summaries.iter().all(|record| record.get("recorded_at").is_none()));

    let repeat = import::preview(&store, &exports("apple"), &options("2026-10-14T12:00:00.000Z")).unwrap();
    assert_eq!(repeat.new_record_count() + repeat.changed_record_count(), 0);
    assert!(repeat.conflicts.is_empty());
    assert!(repeat.matching_record_count() > 100);
}

#[test]
fn a_repeated_record_keeps_ordinals_zero_and_one() {
    let directory = TempDir::new();
    let copy = directory.path().join("copy");
    copy_fixture(&exports("apple"), &copy);
    let xml = std::fs::read_to_string(copy.join("export.xml")).unwrap();
    let line =
        xml.lines().find(|line| line.contains("RestingHeartRate") && line.contains("WHOOP")).unwrap().to_string();
    std::fs::write(copy.join("export.xml"), xml.replace(&line, &format!("{line}\n{line}"))).unwrap();
    let mut store = new_store(directory.path());
    import_path(&mut store, &copy);
    let mut ordinals: Vec<i64> = store
        .connection()
        .prepare("SELECT match_key FROM record_import_keys")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|key| json::parse(&key.unwrap()).unwrap())
        .filter(|key| {
            key[0] == "point_measurement" && key[2] == "resting_heart_rate" && key[3] == "2026-08-09T13:20:00.000Z"
        })
        .map(|key| key[4].as_i64().unwrap())
        .collect();
    ordinals.sort();
    assert_eq!(ordinals, [0, 1]);
    let repeat = import::preview(&store, &copy, &options("2026-10-14T12:00:00.000Z")).unwrap();
    assert!(repeat.conflicts.is_empty());
    assert_eq!(repeat.new_record_count() + repeat.changed_record_count(), 0);
}

#[test]
fn two_writers_at_one_instant_are_two_groups_numbered_from_zero() {
    // 2.1.0 numbered a key after its source was filled in, so each writer's
    // reading is the first of its own group.
    let directory = TempDir::new();
    let xml = concat!(
        "<HealthData>\n",
        " <Record type=\"HKQuantityTypeIdentifierHeartRate\" sourceName=\"Left Watch\" unit=\"count/min\" startDate=\"2026-08-09 08:00:00 -0700\" endDate=\"2026-08-09 08:00:00 -0700\" value=\"61\"/>\n",
        " <Record type=\"HKQuantityTypeIdentifierHeartRate\" sourceName=\"Right Watch\" unit=\"count/min\" startDate=\"2026-08-09 08:00:00 -0700\" endDate=\"2026-08-09 08:00:00 -0700\" value=\"62\"/>\n",
        "</HealthData>\n"
    );
    write_files(&directory.path().join("input"), &[("export.xml", xml)]);
    let mut store = new_store(directory.path());
    import_path(&mut store, &directory.path().join("input"));
    let keys: Vec<Value> = store
        .connection()
        .prepare("SELECT match_key FROM record_import_keys")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|key| json::parse(&key.unwrap()).unwrap())
        .filter(|key| key[0] == "measurement_sample")
        .collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|key| key[4] == 0), "{keys:?}");
}

#[test]
fn a_sample_that_cannot_be_placed_in_time_refuses_the_preview() {
    // 2.1.0 stored this sample at the time of the day's first sample, or
    // stopped with "Invalid time value" when it sorted first. The change is
    // listed in native/reference/deliberate-changes.md.
    let directory = TempDir::new();
    let record = |date: &str, value: u32| {
        format!(
            " <Record type=\"HKQuantityTypeIdentifierHeartRate\" sourceName=\"Watch\" unit=\"count/min\" startDate=\"{date}\" endDate=\"{date}\" value=\"{value}\"/>\n"
        )
    };
    let xml = format!(
        "<HealthData>\n{}{}{}</HealthData>\n",
        record("2026-08-01 08:00:00 -0700", 61),
        record("2026-08-01 25:00:00 -0700", 63),
        record("2026-13-01 08:00:00 -0700", 64)
    );
    write_files(&directory.path().join("input"), &[("export.xml", &xml)]);
    let store = new_store(directory.path());
    let error = import::preview(&store, &directory.path().join("input"), &options(RECEIVED_AT)).unwrap_err();
    assert_eq!(error.to_string(), "Provider timestamp is not an absolute instant: 2026-08-01T25:00:00-07:00");
    assert_eq!(store.database_revision().unwrap(), 0);
}

// ---- Routes ---------------------------------------------------------------------------------

#[test]
fn an_inline_route_stores_points_and_counts_the_ones_it_cannot_store() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &exports("apple-inline-route"), &options(RECEIVED_AT)).unwrap();
    assert_eq!(
        skipped(&preview, "inline WorkoutRoute points we could not store"),
        Some((2, Some("Location latitude \"120\" is outside -90 to 90".into())))
    );
    assert_eq!(skipped(&preview, "inline WorkoutRoute had no location points").map(|skip| skip.0), Some(1));
    preview.apply(&mut store).unwrap();
    let workouts = records(&store, RecordType::WorkoutSession);
    let run = workouts.iter().find(|workout| workout["activity_type"] == "running").unwrap();
    assert_eq!(route_positions(run), [(12.5, 56.5), (12.75, 56.75)]);
    assert_eq!(run["route"][0]["recorded_at"], "2026-06-14T13:01:00.000Z");
    assert_eq!(
        run["route"][0]["input_timestamp"],
        serde_json::json!({
            "timestamp_text": "2026-06-14 08:01:00 -0500",
            "utc_offset_text": "-0500",
            "utc_offset_meaning": "meaning_not_documented",
            "timestamp_basis": "source_offset_timestamp",
        })
    );
    assert_eq!(run["route"][0]["supplied_fields"], serde_json::json!({ "altitude": "200" }));
    assert_eq!(
        run["supplied_fields"]["WorkoutRoute"][0]["MetadataEntry"],
        serde_json::json!([{ "key": "HKMetadataKeySyncVersion", "value": "3" }])
    );
    let walk = workouts.iter().find(|workout| workout["activity_type"] == "walking").unwrap();
    assert!(walk.get("route").is_none());
    assert!(check::check_database(&store).unwrap().valid);
}

#[test]
fn a_zip_with_a_referenced_gpx_stores_that_route() {
    let directory = TempDir::new();
    let fixture = exports("apple-file-reference");
    let zip = directory.path().join("export.zip");
    write_zip(
        &zip,
        &[
            ("apple_health_export/export.xml".into(), std::fs::read_to_string(fixture.join("export.xml")).unwrap()),
            (
                "apple_health_export/workout-routes/route_2026-06-14.gpx".into(),
                std::fs::read_to_string(fixture.join("workout-routes/route_2026-06-14.gpx")).unwrap(),
            ),
        ],
    );
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &zip, &options(RECEIVED_AT)).unwrap();
    assert_eq!(preview.imported, "Apple Health export");
    assert_eq!(preview.route_points_stored, 2);
    assert!(skipped(&preview, "GPX route points we could not store").is_some());
    assert!(skipped(&preview, "inline WorkoutRoute had no location points").is_none());
    preview.apply(&mut store).unwrap();
    assert!(records(&store, RecordType::PointMeasurement)
        .iter()
        .any(|record| record["quantity"] == "resting_heart_rate"));
    let workout = &records(&store, RecordType::WorkoutSession)[0];
    assert_eq!(route_positions(workout), [(13.5, 57.5), (13.75, 57.75)]);
    assert_eq!(workout["route"][0]["recorded_at"], "2026-06-14T13:01:00.000Z");
    assert_eq!(workout["route"][1]["input_timestamp"]["utc_offset_text"], "-05:00");
    assert_eq!(
        workout["supplied_fields"]["WorkoutRoute"],
        serde_json::json!([{
            "creationDate": "2026-06-14 08:05:00 -0500",
            "endDate": "2026-06-14 08:30:00 -0500",
            "sourceName": "Demo Phone",
            "sourceVersion": "1",
            "startDate": "2026-06-14 08:00:00 -0500",
            "FileReference": "/workout-routes/route_2026-06-14.gpx",
            "MetadataEntry": [{ "key": "HKMetadataKeySyncVersion", "value": "4" }],
        }])
    );
    assert!(check::check_database(&store).unwrap().valid);
}

#[test]
fn a_route_file_outside_the_workout_or_named_twice_is_not_stored() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &exports("apple-route-mismatch"), &options(RECEIVED_AT)).unwrap();
    assert_eq!(
        skipped(&preview, "workout route file is outside the workout's time"),
        Some((1, Some("workout-routes/route_2026-07-01.gpx".into())))
    );
    assert_eq!(
        skipped(&preview, "workout route file is already stored on another workout"),
        Some((1, Some("workout-routes/route_2026-08-02.gpx".into())))
    );
    assert_eq!(skipped(&preview, "GPX route points we could not store").map(|skip| skip.0), Some(1));
    assert!(skipped(&preview, "workout routes with no workout in the same window").is_none());
    preview.apply(&mut store).unwrap();
    let workouts = records(&store, RecordType::WorkoutSession);
    let by_activity = |activity: &str| workouts.iter().find(|workout| workout["activity_type"] == activity).unwrap();
    assert!(by_activity("running").get("route").is_none());
    assert_eq!(route_positions(by_activity("cycling")), [(15.5, 59.5), (15.75, 59.75)]);
    assert!(by_activity("walking").get("route").is_none());
}

#[test]
fn a_bare_xml_reads_only_the_folders_beside_it() {
    let directory = TempDir::new();
    let root = directory.path().join("input");
    let near = gpx_at("16.5", "60.5", "2026-06-14T08:01:00-05:00", "2026-06-14T08:02:00-05:00");
    let far = gpx_at("17.5", "61.5", "2026-06-14T08:01:00-05:00", "2026-06-14T08:02:00-05:00");
    write_files(
        &root,
        &[
            ("export.xml", &workout_xml(&["/workout-routes/near.gpx"])),
            ("workout-routes/near.gpx", &near),
            ("other/workout-routes/near.gpx", &far),
            ("other/electrocardiograms/ecg_2026-06-14.csv", "not an ecg\n"),
        ],
    );
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &root.join("export.xml"), &options(RECEIVED_AT)).unwrap();
    assert!(skipped(&preview, "ECG recordings we could not read").is_none());
    preview.apply(&mut store).unwrap();
    let workout = &records(&store, RecordType::WorkoutSession)[0];
    assert_eq!(route_positions(workout), [(16.5, 60.5), (16.5, 60.5)]);
}

#[test]
fn a_missing_route_file_is_named() {
    let directory = TempDir::new();
    let root = directory.path().join("input");
    write_files(&root, &[("export.xml", &workout_xml(&["/workout-routes/missing.gpx"]))]);
    let store = new_store(directory.path());
    let preview = import::preview(&store, &root.join("export.xml"), &options(RECEIVED_AT)).unwrap();
    assert_eq!(
        skipped(&preview, "workout route file named by a WorkoutRoute was not in the export"),
        Some((1, Some("workout-routes/missing.gpx".into())))
    );
}

#[test]
fn two_route_files_on_one_workout_are_both_stored() {
    let directory = TempDir::new();
    let root = directory.path().join("input");
    write_files(
        &root,
        &[
            ("export.xml", &workout_xml(&["/workout-routes/first.gpx", "/workout-routes/second.gpx"])),
            (
                "workout-routes/first.gpx",
                &gpx_at("18.5", "62.5", "2026-06-14T08:01:00-05:00", "2026-06-14T08:02:00-05:00"),
            ),
            (
                "workout-routes/second.gpx",
                &gpx_at("18.75", "62.75", "2026-06-14T08:10:00-05:00", "2026-06-14T08:11:00-05:00"),
            ),
        ],
    );
    let mut store = new_store(directory.path());
    let preview = import::preview(&store, &root.join("export.xml"), &options(RECEIVED_AT)).unwrap();
    assert!(skipped(&preview, "workout route file is already stored on another workout").is_none());
    assert!(skipped(&preview, "workout routes with no workout in the same window").is_none());
    preview.apply(&mut store).unwrap();
    let workout = &records(&store, RecordType::WorkoutSession)[0];
    assert_eq!(route_positions(workout), [(18.5, 62.5), (18.5, 62.5), (18.75, 62.75), (18.75, 62.75)]);
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_inside_a_route_folder_refuses_the_export() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new();
    let root = directory.path().join("input");
    write_files(&root, &[("export.xml", &workout_xml(&["/workout-routes/near.gpx"])), ("outside.gpx", "x")]);
    std::fs::create_dir_all(root.join("workout-routes")).unwrap();
    symlink(root.join("outside.gpx"), root.join("workout-routes/near.gpx")).unwrap();
    let store = new_store(directory.path());
    let error = import::preview(&store, &root.join("export.xml"), &options(RECEIVED_AT)).unwrap_err();
    assert_eq!(error.to_string(), "refusing symbolic link in provider export: workout-routes/near.gpx");
    let error = import::preview(&store, &root, &options(RECEIVED_AT)).unwrap_err();
    assert_eq!(error.to_string(), "refusing symbolic link in provider export: workout-routes/near.gpx");
}

#[test]
fn a_zip_holding_settings_export_xml_is_not_an_apple_export() {
    let directory = TempDir::new();
    let zip = directory.path().join("activities.zip");
    write_zip(
        &zip,
        &[
            ("settings_export.xml".into(), "<settings/>".into()),
            ("activity.gpx".into(), gpx_at("18.5", "62.5", "2026-06-14T08:01:00-05:00", "2026-06-14T08:02:00-05:00")),
        ],
    );
    assert_eq!(import::detect::detect_shared_import(&zip).unwrap(), Some(import::SharedImport::Garmin));
}

#[test]
fn the_fixture_route_has_points_elevation_and_a_partial_last_split() {
    let text = std::fs::read_to_string(exports("apple/workout-routes/route_2026-08-09_7.00pm.gpx")).unwrap();
    let points = routes::parse_gpx(&text);
    assert!(points.len() > 100);
    assert!((points[0].latitude - 37.77).abs() < 0.05);
    assert!(points[0].elevation_m.is_some());
    let summary = routes::summarize_route(points).unwrap();
    let last = summary.splits.last().unwrap();
    assert!(last.label.contains("partial"));
    assert!(last.distance_m < 1000.0);
}

// ---- Numbers 2.1.0 calculated ----------------------------------------------------------

#[test]
fn haversine_distances_match_v8_bit_for_bit() {
    let golden = golden("route-numerics.json");
    for case in golden["haversine"].as_array().unwrap() {
        let input: Vec<f64> = case["input"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect();
        let expected = case["metres"].as_f64().unwrap();
        let actual = routes::haversine_m(input[0], input[1], input[2], input[3]);
        assert_eq!(actual.to_bits(), expected.to_bits(), "{input:?}: {actual} != {expected}");
    }
}

#[test]
fn route_summaries_match_2_1_0() {
    let golden = golden("route-numerics.json");
    for case in golden["summaries"].as_array().unwrap() {
        let summary = routes::summarize_route(routes::parse_gpx(case["gpx"].as_str().unwrap())).unwrap();
        let expected = &case["summary"];
        assert_eq!(Some(summary.start_ms), instant::parse_instant(expected["start"].as_str().unwrap()));
        assert_eq!(Some(summary.end_ms), instant::parse_instant(expected["end"].as_str().unwrap()));
        assert_eq!(summary.distance_m, expected["distanceM"].as_f64().unwrap());
        assert_eq!(summary.elevation_gain_m, expected["elevationGainM"].as_f64().unwrap());
        let splits: Vec<Value> = summary
            .splits
            .iter()
            .map(|split| serde_json::json!({ "label": split.label, "duration_s": split.duration_s, "distance_m": split.distance_m }))
            .collect();
        assert_eq!(json::to_sorted_string(&Value::Array(splits)), json::to_sorted_string(&expected["splits"]));
    }
}

#[test]
fn ecg_beats_peaks_and_rmssd_match_2_1_0_with_noise_drift_and_artifacts() {
    let numbers = golden("beat-numerics.json");
    let edges = golden("watch-edges.json");
    let files = edges["steps"][0]["input"]["files"].as_object().unwrap();
    for case in numbers["recordings"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let text = files[name].as_str().unwrap();
        let recording = match apple::parse_ecg_csv(text) {
            Ok(recording) => recording,
            Err(apple::EcgProblem::Unreadable(detail)) => {
                assert_eq!(case["parsed"]["kind"], "unreadable", "{name}");
                assert_eq!(case["parsed"]["detail"], detail.as_str(), "{name}");
                continue;
            }
            Err(other) => panic!("{name}: {other:?}"),
        };
        assert_eq!(recording.microvolts.len() as u64, case["sample_count"].as_u64().unwrap(), "{name}");
        let peaks = rpeaks::detect_r_peaks(&recording.microvolts, recording.sample_rate_hz);
        let expected: Vec<u64> =
            case["peaks"]["peaks"].as_array().unwrap().iter().map(|peak| peak.as_u64().unwrap()).collect();
        assert_eq!(peaks.peaks.iter().map(|peak| *peak as u64).collect::<Vec<_>>(), expected, "{name}");
        assert_eq!(peaks.quality, case["peaks"]["quality"].as_f64().unwrap(), "{name}");
        match apple::beats_from_ecg(&recording) {
            Ok(beats) => {
                let intervals: Vec<f64> = case["beats"]["intervals_ms"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap())
                    .collect();
                assert_eq!(
                    beats.intervals_ms.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
                    intervals.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
                    "{name}"
                );
                let rmssd = hrv::rmssd_from_intervals(&beats.intervals_ms);
                let expected = &case["beats"]["rmssd"];
                match rmssd {
                    None => assert!(expected.is_null(), "{name}"),
                    Some(rmssd) => {
                        assert_eq!(rmssd.rmssd_ms, expected["rmssd_ms"].as_f64().unwrap(), "{name}");
                        assert_eq!(rmssd.n_beats as u64, expected["n_beats"].as_u64().unwrap(), "{name}");
                        assert_eq!(rmssd.n_dropped as u64, expected["n_dropped"].as_u64().unwrap(), "{name}");
                        assert_eq!(rmssd.window_s, expected["window_s"].as_f64().unwrap(), "{name}");
                    }
                }
            }
            Err(apple::EcgProblem::NotSinus(detail) | apple::EcgProblem::TooFewBeats(detail)) => {
                assert_eq!(case["beats"]["detail"], detail.as_str(), "{name}");
            }
            Err(other) => panic!("{name}: {other:?}"),
        }
    }
}

// ---- ECG fixture behavior ----------------------------------------------------------------

#[test]
fn the_fixture_ecgs_read_their_headers_and_gate_on_rhythm() {
    let folder = exports("apple/electrocardiograms");
    let sinus = std::fs::read_to_string(folder.join("ecg_2026-08-09.csv")).unwrap();
    assert!(sinus.contains("Device,\"Watch7,2\""));
    let recording = apple::parse_ecg_csv(&sinus).unwrap();
    assert_eq!(recording.recorded_at, "2026-08-09T07:20:11-07:00");
    assert_eq!(recording.classification, "Sinus Rhythm");
    assert!((recording.sample_rate_hz - 510.844).abs() < 1e-9);
    assert!(recording.microvolts.len() > 14_000);
    let beats = apple::beats_from_ecg(&recording).unwrap();
    // The fixture alternates 880 and 920 ms, so the true RMSSD is 40.
    let rmssd = hrv::rmssd_from_intervals(&beats.intervals_ms).unwrap();
    assert!(rmssd.rmssd_ms > 36.0 && rmssd.rmssd_ms < 44.0);
    assert!(beats.quality > 1.0);
    let afib = apple::parse_ecg_csv(&std::fs::read_to_string(folder.join("ecg_2026-08-07.csv")).unwrap()).unwrap();
    match apple::beats_from_ecg(&afib) {
        Err(apple::EcgProblem::NotSinus(detail)) => assert!(detail.contains("Atrial Fibrillation")),
        other => panic!("{other:?}"),
    }
}

// ---- The parity snapshot ----------------------------------------------------------------

fn source_label(source: &Value) -> String {
    if source["source_type"] == "manual" {
        return "manual".into();
    }
    format!(
        "{}|{}|{}|{}|{}",
        source["source_type"].as_str().unwrap(),
        source["data_writer"].as_str().unwrap(),
        source["vendor"].as_str().unwrap_or_default(),
        source["sensor_type"].as_str().unwrap_or_default(),
        source["data_route"].as_str().unwrap()
    )
}

/// Equal as JSON values, so `51` and `51.0` are one number.
fn same(left: &Value, right: &Value, context: &str) {
    assert!(json::semantic_eq(left, right), "{context}: {left} != {right}");
}

fn utc(text: &str) -> String {
    instant::normalized_utc(text).unwrap()
}

/// The summaries 2.1.0 stored for a signal's aggregates, by summary name.
fn summary_values(record: &Value) -> BTreeMap<String, f64> {
    record["source_reported_summaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|summary| {
            (summary["summary_type"].as_str().unwrap().to_string(), summary["standard_value"].as_f64().unwrap())
        })
        .collect()
}

fn aggregate_values(aggregates: &Value) -> BTreeMap<String, f64> {
    let names = athleticstandard::import::candidate::SUMMARY_NAMES;
    aggregates
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(name, value)| {
            let (_, summary_type, _) = names.iter().find(|(known, _, _)| known == name)?;
            Some((summary_type.to_string(), value.as_f64()?))
        })
        .collect()
}

#[test]
fn the_stored_records_carry_everything_the_2_1_0_parity_snapshot_lists() {
    let parity =
        json::parse(&std::fs::read_to_string(repository_root().join("tests/fixtures/apple-rust-parity.json")).unwrap())
            .unwrap();
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import_path(&mut store, &exports("apple"));

    let sources = records(&store, RecordType::Source);
    let label_of = |id: &str| source_label(sources.iter().find(|source| source["record_id"] == id).unwrap());
    let mut stored_labels: Vec<String> = sources.iter().map(source_label).collect();
    let mut expected_labels: Vec<String> = parity["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| source["label"].as_str().unwrap().to_string())
        .collect();
    stored_labels.sort();
    expected_labels.sort();
    assert_eq!(stored_labels, expected_labels);

    let points = records(&store, RecordType::PointMeasurement);
    let sleeps = records(&store, RecordType::SleepSession);
    let workouts = records(&store, RecordType::WorkoutSession);
    for hard in parity["hard"].as_array().unwrap() {
        let signal = &hard["signal"];
        let receipt = &hard["receipt"];
        let source = hard["source"].as_str().unwrap();
        match signal["type"].as_str().unwrap() {
            "sleep_session" | "workout_session" => {
                let list = if signal["type"] == "sleep_session" { &sleeps } else { &workouts };
                let start = utc(signal["start"].as_str().unwrap());
                let record = list
                    .iter()
                    .find(|record| {
                        record["start_at"] == start.as_str()
                            && label_of(record["source_id"].as_str().unwrap()) == source
                    })
                    .unwrap_or_else(|| panic!("no session at {start} from {source}"));
                assert_eq!(record["end_at"], utc(signal["end"].as_str().unwrap()).as_str());
                assert_eq!(record["input_start_timestamp"], receipt["input_start_timestamp"]);
                assert_eq!(record["input_end_timestamp"], receipt["input_end_timestamp"]);
                assert_eq!(summary_values(record), aggregate_values(&signal["aggregates"]));
                if let Some(activity) = signal["aggregates"].get("activity") {
                    assert_eq!(&record["activity_type"], activity);
                }
                if let Some(segments) = signal.get("segments") {
                    let labels: Vec<&Value> =
                        record["segments"].as_array().unwrap().iter().map(|segment| &segment["label"]).collect();
                    let expected: Vec<&Value> =
                        segments.as_array().unwrap().iter().map(|segment| &segment["label"]).collect();
                    assert_eq!(labels, expected);
                }
            }
            quantity => {
                let at = utc(signal["recorded_at"].as_str().unwrap());
                let record = points
                    .iter()
                    .find(|record| {
                        record["quantity"] == quantity
                            && record["recorded_at"] == at.as_str()
                            && label_of(record["source_id"].as_str().unwrap()) == source
                    })
                    .unwrap_or_else(|| panic!("no {quantity} at {at} from {source}"));
                same(&record["value"]["standard_value"], &signal["value"], quantity);
                same(&record["value"]["standard_unit"], &signal["unit"], quantity);
                same(&record["input_timestamp"], &receipt["input_timestamp"], quantity);
                for key in ["input_value", "input_unit", "input_value_missing_reason", "input_unit_missing_reason"] {
                    if let Some(expected) = receipt.get(key) {
                        same(&record["value"][key], expected, key);
                    }
                }
                if let Some(derived) = signal.get("derived") {
                    let calculation = &record["calculations"][0];
                    same(&calculation["from_quantity"], &derived["from"], "calculation");
                    same(&calculation["method"], &derived["method"], "calculation");
                    same(&calculation["window_s"], &derived["window_s"], "calculation");
                    same(&calculation["usable_sample_count"], &derived["n_beats"], "calculation");
                    same(
                        calculation.get("signal_quality").unwrap_or(&Value::Null),
                        derived.get("signal_quality").unwrap_or(&Value::Null),
                        "signal_quality",
                    );
                }
            }
        }
    }

    let summaries = records(&store, RecordType::ProviderSummary);
    for item in parity["summaries"].as_array().unwrap() {
        let summary = &item["summary"];
        let record = summaries.iter().find(|record| record["summary_type"] == summary["summary_type"]).unwrap();
        assert_eq!(label_of(record["source_id"].as_str().unwrap()), item["source"].as_str().unwrap());
        same(&record["value"]["standard_value"], &summary["value"], "summary value");
        same(&record["value"]["input_value"], &summary["input_value"], "summary input");
        same(&record["input_timestamp"], &summary["input_timestamp"], "summary time");
        same(&record["provider_reporting_periods"], &summary["provider_reporting_periods"], "summary periods");
    }

    let samples = records(&store, RecordType::MeasurementSample);
    for series in parity["series"].as_array().unwrap() {
        let quantity = series["quantity"].as_str().unwrap();
        let source = series["source"].as_str().unwrap();
        let stored: Vec<&Value> = samples
            .iter()
            .filter(|sample| {
                sample["quantity"] == quantity && label_of(sample["source_id"].as_str().unwrap()) == source
            })
            .collect();
        assert_eq!(stored.len() as u64, series["n"].as_u64().unwrap(), "{quantity} {source}");
        for sample in series["samples"].as_array().unwrap() {
            let at = utc(sample["at"].as_str().unwrap());
            assert!(
                stored.iter().any(|record| record["recorded_at"] == at.as_str()
                    && json::semantic_eq(&record["value"]["standard_value"], &sample["value"])),
                "{quantity} sample at {at}"
            );
        }
    }

    let expected_skips: Vec<(String, u64)> = parity["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| (item[0].as_str().unwrap().to_string(), item[1].as_u64().unwrap()))
        .collect();
    for (reason, count) in expected_skips {
        assert_eq!(skipped(&preview, &reason).map(|skip| skip.0), Some(count), "{reason}");
    }
    for example in parity["skip_examples"].as_array().unwrap() {
        let reason = example[0].as_str().unwrap();
        assert_eq!(skipped(&preview, reason).and_then(|skip| skip.1).as_deref(), example[1].as_str(), "{reason}");
    }

    let devices = records(&store, RecordType::SourceDevice);
    for seen in parity["devices"].as_array().unwrap() {
        let device = devices.iter().find(|device| device["hardware"] == seen["device"]["hardware"]).unwrap();
        assert_eq!(label_of(device["source_id"].as_str().unwrap()), seen["source"].as_str().unwrap());
        same(&device["coverage_start_date"], &seen["from"], "device");
        same(&device["coverage_end_date"], &seen["to"], "device");
        for field in ["name", "manufacturer", "model"] {
            same(&device[field], &seen["device"][field], "device");
        }
    }
}
