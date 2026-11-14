//! Mapped imports compared with 2.1.0.
//!
//! The scenarios in `native/tests/golden/routes/{aggregator,connector,invented}-*.json`
//! were written by `native/tools/reference/route-golden.ts` from the 2.1.0
//! helper. `route_support::keyed` names the normalizations. The other tests
//! port `tests/mapping-cli.test.ts` and `tests/mapping-path.test.ts`.

mod import_support;
mod route_support;

use std::collections::HashMap;
use std::path::Path;

use athleticstandard::assets::{self, AssetOrigin};
use athleticstandard::import::{self, mapping, ImportOptions};
use athleticstandard::json::{self, Value};
use athleticstandard::model::RecordActor;
use athleticstandard::store::{Changes, Deletion, Store};
use import_support::{new_store, TempDir};
use route_support::exports;
use route_support::keyed::{run_keyed_scenario, step_time};

const S_RING: &str = "ring|invented-ring|invented-app";
const S_WATCH: &str = "watch|invented-watch|invented-app";
const S_PHONE: &str = "phone|iPhone15,2|Health";
const S_AWATCH: &str = "watch|Watch6,1|Health";
const S_JORA: &str = "ring|ring-j1|oura-app";
const S_JAPPLE: &str = "watch|watch-a|health";

fn options(received_at: &str) -> ImportOptions {
    ImportOptions { received_at: Some(received_at.to_string()) }
}

fn shipped(name: &str) -> mapping::Mapping {
    mapping::load_mapping(assets::shipped_mapping(name).unwrap()).unwrap()
}

/// Preview, then write, and return both results. They differ only in `record_ids`.
fn mapped(store: &mut Store, input: &Path, mapping: &mapping::Mapping, step: usize) -> Value {
    let preview = import::preview_mapped(store, input, mapping, &options(&step_time(step))).unwrap();
    let previewed = preview.report_value(false).unwrap();
    preview.apply(store).unwrap();
    let written = preview.report_value(true).unwrap();
    assert_eq!(previewed["record_ids"], serde_json::json!([]));
    let mut same = previewed.clone();
    same["record_ids"] = written["record_ids"].clone();
    assert_eq!(same, written);
    written
}

fn key(parts: &[&str]) -> String {
    parts.iter().map(|part| mapping::key_part(part)).collect::<Vec<_>>().join(":")
}

struct Stored {
    record_id: String,
    record_type: String,
    current_revision: i64,
    method: String,
    json: String,
    data: Value,
}

fn stored(store: &Store) -> HashMap<String, Stored> {
    let mut statement = store
        .connection()
        .prepare(
            "SELECT k.match_key_method, k.match_key, r.record_id, r.record_type, r.current_revision, rv.record_json
               FROM record_import_keys k
               JOIN records r ON r.record_id = k.record_id
               JOIN record_revisions rv ON rv.record_id = r.record_id AND rv.revision = r.current_revision",
        )
        .unwrap();
    statement
        .query_map([], |row| {
            let json: String = row.get(5)?;
            Ok((
                row.get::<_, String>(1)?,
                Stored {
                    method: row.get(0)?,
                    record_id: row.get(2)?,
                    record_type: row.get(3)?,
                    current_revision: row.get(4)?,
                    data: json::parse(&json).unwrap(),
                    json,
                },
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn source(store: &Store, id: &str) -> Value {
    let text: String = store
        .connection()
        .query_row("SELECT data_json FROM sources WHERE record_id = ?1", [id], |row| row.get(0))
        .unwrap();
    json::parse(&text).unwrap()
}

fn strings(value: &Value, field: &str) -> Vec<String> {
    value[field].as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect()
}

fn record_count(store: &Store) -> i64 {
    store.connection().query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap()
}

#[test]
fn the_aggregator_mapping_matches_2_1_0() {
    for name in ["aggregator-summaries", "aggregator-pages", "aggregator-deleted-source", "aggregator-deleted-record"] {
        run_keyed_scenario(name);
    }
}

#[test]
fn the_connector_mapping_matches_2_1_0() {
    run_keyed_scenario("connector-summaries");
}

#[test]
fn invented_mapping_files_match_2_1_0() {
    run_keyed_scenario("invented-mappings");
}

#[test]
fn mapping_refusals_match_2_1_0() {
    let directory = route_support::golden("aggregator-pages.json");
    assert!(directory["steps"].is_array());
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/routes");
    let mut names: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("invented-refusal-"))
        .collect();
    names.sort();
    assert_eq!(names.len(), 20);
    for name in names {
        run_keyed_scenario(name.trim_end_matches(".json"));
    }
}

/// `open_wearables_fixture_maps_fields`.
#[test]
fn the_aggregator_fixture_maps_fields() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let result = mapped(&mut store, &exports().join("open-wearables/summaries.json"), &shipped("open-wearables"), 0);
    let by_key = stored(&store);

    let night = &by_key[&key(&["oura", S_RING, "sleep", "ow-sleep-1"])];
    assert_eq!(night.method, "open_wearables/v1");
    assert_eq!(night.record_type, "sleep_session");
    assert_eq!(night.data["start_at"], "2026-08-09T06:00:00.000Z");
    assert_eq!(night.data["end_at"], "2026-08-09T14:00:00.000Z");
    assert_eq!(
        night.data["input_start_timestamp"],
        serde_json::json!({
            "timestamp_text": "2026-08-09T02:00:00.000-04:00",
            "utc_offset_text": "-04:00",
            "utc_offset_meaning": "meaning_not_documented",
            "timestamp_basis": "source_offset_timestamp"
        })
    );
    assert_eq!(night.data["asleep_seconds"], 600);
    assert_eq!(night.data["in_bed_seconds"], 700);
    assert_eq!(night.data["stage_minutes"], serde_json::json!({ "awake": 2, "light": 3, "deep": 4 }));
    assert_eq!(
        night.data["supplied_fields"],
        serde_json::json!({ "zone_offset": "-04:00", "duration_seconds": 800, "efficiency_percent": 91, "is_nap": false })
    );
    assert!(night.json.contains("\"asleep_seconds\":600,"));
    let ring = source(&store, night.data["source_id"].as_str().unwrap());
    assert_eq!(ring["data_writer"], "oura");
    assert_eq!(ring["data_route"], "open_wearables");
    assert_eq!(ring["vendor"], "oura");
    assert_eq!(ring["source_type"], "export_file");
    assert_eq!(ring["sensor_type"], S_RING);
    let device = &by_key[&key(&["oura", S_RING, "source_device"])];
    assert_eq!(device.data["source_id"], night.data["source_id"]);
    assert_eq!(device.data["name"], "Invented Ring");
    assert_eq!(device.data["model"], "invented-ring");
    assert_eq!(device.data["hardware"], "ring-1");
    assert_eq!(device.data["manufacturer"], "oura");

    let workout = &by_key[&key(&["garmin", S_WATCH, "workout", "ow-workout-1"])];
    assert_eq!(workout.data["activity_type"], "running");
    assert_eq!(
        workout.data["field_basis"],
        serde_json::json!({ "activity_type": "aggregator_normalized", "value": "provider_or_aggregator" })
    );
    assert_eq!(workout.data.get("maximum_heart_rate_bpm"), None);
    let peak = &by_key[&key(&["garmin", S_WATCH, "workout", "ow-workout-1", "summary", "max_heart_rate"])];
    assert_eq!(peak.data["value"]["standard_value"], 180);
    assert_eq!(peak.data["field_basis"], serde_json::json!({ "value": "provider_or_aggregator" }));
    assert!(strings(&result, "left_out")
        .contains(&"workout ow-workout-manual: entry_source manual is a personal report".to_string()));

    let at = "2026-08-09T06:12:00.000Z";
    for (kind, quantity, value, unit) in [
        ("heart_rate_variability_rmssd", "hrv_rmssd", 42.5, "ms"),
        ("heart_rate_variability_sdnn", "hrv_sdnn", 61.2, "ms"),
        ("resting_heart_rate", "resting_heart_rate", 48.0, "bpm"),
        ("respiratory_rate", "respiratory_rate", 16.0, "brpm"),
        ("skin_temperature", "skin_temperature", 33.2, "°C"),
        ("oxygen_saturation", "oxygen_saturation", 98.0, "%"),
        ("heart_rate_recovery_one_minute", "hr_recovery", 22.0, "bpm"),
    ] {
        let point = &by_key[&key(&["oura", S_RING, "timeseries", "point", kind, at])];
        assert_eq!(point.data["quantity"], quantity, "{kind}");
        assert_eq!(point.data["value"]["standard_value"].as_f64(), Some(value), "{kind}");
        assert_eq!(point.data["value"]["standard_unit"], unit, "{kind}");
        assert_eq!(point.data.get("supplied_fields"), None, "{kind}");
    }
    let wrist = &by_key[&key(&["apple", S_AWATCH, "timeseries", "point", "skin_temperature", at])];
    assert_eq!(wrist.data["quantity"], "wrist_temperature_sleeping");
    let stress = &by_key[&key(&["garmin", S_WATCH, "timeseries", "score", "garmin_stress_level", at])];
    assert_eq!(stress.record_type, "provider_score");
    assert_eq!(stress.data["scale"], "1-100");
    assert_eq!(stress.data["field_basis"], serde_json::json!({ "value": "provider" }));
    let phone =
        &by_key[&key(&["apple", S_PHONE, "timeseries", "point", "resting_heart_rate", "2026-08-09T08:00:00.000Z"])];
    let watch =
        &by_key[&key(&["apple", S_AWATCH, "timeseries", "point", "resting_heart_rate", "2026-08-09T08:00:00.000Z"])];
    assert_ne!(phone.data["source_id"], watch.data["source_id"]);
    for kind in ["dietary_caffeine", "workout_effort_score", "skin_temperature_deviation"] {
        assert!(strings(&result, "left_out").contains(&format!("timeseries {kind} {at}: unmapped type {kind}")));
    }
}

/// `every mapped value states its basis (D187)`.
#[test]
fn every_mapped_value_states_its_basis() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    mapped(&mut store, &exports().join("open-wearables/summaries.json"), &shipped("open-wearables"), 0);
    mapped(&mut store, &exports().join("junction/summaries.json"), &shipped("junction"), 1);
    for (match_key, record) in stored(&store) {
        if record.record_type == "source_device" {
            assert_eq!(record.data.get("field_basis"), None, "{match_key}");
            continue;
        }
        let expected = if record.record_type == "provider_score" {
            "provider"
        } else if match_key.ends_with(":point:hrv_rmssd") && match_key.contains(":sleep:") {
            "hrv_mean_rmssd"
        } else {
            "provider_or_aggregator"
        };
        assert_eq!(record.data["field_basis"]["value"], expected, "{match_key}");
    }
    let junction_night = &stored(&store)[&key(&["oura", S_JORA, "sleep", "jn-sleep-1"])];
    assert_eq!(junction_night.data["field_basis"], serde_json::json!({ "value": "provider_or_aggregator" }));
    let apple = &stored(&store)[&key(&["apple_health_kit", S_JAPPLE, "sleep", "jn-sleep-apple"])];
    assert_eq!(apple.data["supplied_fields"], serde_json::json!({ "duration": 3600, "average_hrv": 33 }));
}

/// `rerun_does_not_add_a_record`.
#[test]
fn a_rerun_matches_and_a_change_keeps_the_first_received_time() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let input = exports().join("open-wearables/summaries.json");
    let mapping = shipped("open-wearables");
    let first_outcome = mapped(&mut store, &input, &mapping, 0);
    assert_eq!(first_outcome["outcome"], serde_json::json!({ "new": 22, "changed": 0, "unchanged": 0 }));
    let night_key = key(&["oura", S_RING, "sleep", "ow-sleep-1"]);
    let first = stored(&store);
    let count = record_count(&store);
    let again = mapped(&mut store, &input, &mapping, 1);
    assert_eq!(again["outcome"], serde_json::json!({ "new": 0, "changed": 0, "unchanged": 22 }));
    assert_eq!(record_count(&store), count);
    assert_eq!(stored(&store)[&night_key].current_revision, 1);

    let mut changed: Value = json::parse(&std::fs::read_to_string(&input).unwrap()).unwrap();
    changed["sleep"][0]["end_time"] = Value::String("2026-08-09T10:05:00.000-04:00".into());
    let copy = directory.path().join("changed.json");
    std::fs::write(&copy, json::to_string(&changed)).unwrap();
    mapped(&mut store, &copy, &mapping, 2);
    let revised = &stored(&store)[&night_key];
    assert_eq!(revised.record_id, first[&night_key].record_id);
    assert_eq!(revised.current_revision, 2);
    assert_eq!(revised.data["end_at"], "2026-08-09T14:05:00.000Z");
    assert_eq!(revised.data["received_at"], first[&night_key].data["received_at"]);
}

/// `a failure partway through an import writes nothing`, and the D191 names.
#[test]
fn a_bad_row_refuses_the_whole_import_in_preview_and_write() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mapping = mapping::load_mapping(
        "```ath-mapping\nname: invented\nroute: invented_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\nasleep: asleep_seconds s s\n```\n",
    )
    .unwrap();
    let night = |id: &str, day: &str, extra: &str| {
        format!(
            r#"{{"id":"{id}","start_time":"2026-08-{day}T01:00:00.000Z","end_time":"2026-08-{day}T07:00:00.000Z","source":{{"provider":"invented-maker"}},"asleep":20000{extra}}}"#
        )
    };
    let good = directory.path().join("good.json");
    std::fs::write(&good, format!(r#"{{"sleep":[{}]}}"#, night("good-night", "09", ""))).unwrap();
    mapped(&mut store, &good, &mapping, 0);
    let count = record_count(&store);
    for (extra, reason) in [
        (r#","":"no name""#, "a supplied field has an empty name, and the format needs a name for every field"),
        (r#","__proto__":{"polluted":true}"#, "a supplied field is named __proto__, which JavaScript readers drop"),
        (r#","extra":{"__proto__":1}"#, "a supplied field is named __proto__, which JavaScript readers drop"),
    ] {
        let bad = directory.path().join("bad.json");
        std::fs::write(
            &bad,
            format!(r#"{{"sleep":[{},{}]}}"#, night("next-night", "10", ""), night("bad-night", "11", extra)),
        )
        .unwrap();
        let error = import::preview_mapped(&store, &bad, &mapping, &options(&step_time(1))).unwrap_err();
        assert_eq!(error.to_string(), format!("sleep_session bad-night: {reason}"));
        assert_eq!(record_count(&store), count);
    }
}

/// `a deleted mapped source is left out` with the exact counts.
#[test]
fn a_deleted_source_leaves_its_rows_out_and_makes_no_second_source() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let input = exports().join("open-wearables/summaries.json");
    let mapping = shipped("open-wearables");
    mapped(&mut store, &input, &mapping, 0);
    let before = stored(&store);
    let ring_id: String = store
        .connection()
        .query_row("SELECT record_id FROM sources WHERE data_writer = 'oura' AND sensor_type = ?1", [S_RING], |row| {
            row.get(0)
        })
        .unwrap();
    store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: ring_id.clone(),
                expected_revision: 1,
                deleted_by: RecordActor::Athlete,
                deleted_at: Some(step_time(1)),
            }],
            ..Changes::default()
        })
        .unwrap();
    let sources_before: i64 =
        store.connection().query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0)).unwrap();
    let written = mapped(&mut store, &input, &mapping, 1);
    let gone = format!("its source {ring_id} is deleted");
    let left_out = strings(&written, "left_out");
    assert!(left_out.contains(&format!("sleep ow-sleep-1: {gone}")));
    assert!(left_out.contains(&format!("timeseries heart_rate_variability_rmssd 2026-08-09T06:12:00.000Z: {gone}")));
    let kept = before.values().filter(|record| record.data["source_id"] != ring_id.as_str()).count();
    assert_eq!(written["outcome"], serde_json::json!({ "new": 0, "changed": 0, "unchanged": kept }));
    let sources_after: i64 =
        store.connection().query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0)).unwrap();
    assert_eq!(sources_after, sources_before);
}

/// `the first line that writes a field wins, so importing again changes nothing`.
#[test]
fn the_first_line_that_writes_a_field_wins() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mapping = mapping::load_mapping(
        "```ath-mapping\nname: two-lines\nroute: invented_route\nwriter_field: source.provider\nvalue_basis: provider\n\n[sleep]\nid_field: id\nstart_field: start_time\nend_field: end_time\nrmssd_first: hrv_rmssd ms ms\nrmssd_second: hrv_rmssd ms ms\nasleep_total: asleep_seconds s s when-absent\nasleep: asleep_seconds s s\n```",
    )
    .unwrap();
    let input = directory.path().join("nights.json");
    std::fs::write(
        &input,
        r#"{"sleep":[{"id":"n-1","start_time":"2026-08-09T01:00:00.000Z","end_time":"2026-08-09T07:00:00.000Z","source":{"provider":"invented-maker"},"asleep":20000,"asleep_total":21000,"rmssd_first":40,"rmssd_second":44}]}"#,
    )
    .unwrap();
    assert_eq!(
        mapped(&mut store, &input, &mapping, 0)["outcome"],
        serde_json::json!({ "new": 2, "changed": 0, "unchanged": 0 })
    );
    assert_eq!(
        mapped(&mut store, &input, &mapping, 1)["outcome"],
        serde_json::json!({ "new": 0, "changed": 0, "unchanged": 2 })
    );
    let session = &stored(&store)[&key(&["invented-maker", "-", "sleep", "n-1"])];
    assert_eq!(session.data["asleep_seconds"], 20000);
    assert_eq!(session.data["supplied_fields"], serde_json::json!({ "asleep_total": 21000, "rmssd_second": 44 }));
}

/// `installed_mapping_file_is_the_one_that_runs`, `person_adds_my_source`, and
/// `tests/mapping-path.test.ts`: the installed file wins over the shipped one.
#[test]
fn an_installed_mapping_file_wins_and_paths_are_named_clearly() {
    let directory = TempDir::new();
    let cwd = directory.path();
    let installed = cwd.join(".agents/mappings/open-wearables.md");
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    let edited = assets::shipped_mapping("open-wearables")
        .unwrap()
        .replace("type.heart_rate_variability_rmssd: hrv_rmssd ms ms", "type.LastNightAvg: hrv_rmssd ms ms");
    std::fs::write(&installed, &edited).unwrap();
    let resolved = assets::resolve_mapping("open-wearables", cwd).unwrap();
    assert_eq!(resolved.origin, AssetOrigin::Installed);
    assert_eq!(
        mapping::load_mapping(&resolved.text).unwrap_err().to_string(),
        "refusing to map LastNightAvg onto hrv_rmssd; the field does not name the statistic"
    );
    let shipped = assets::resolve_mapping("junction", cwd).unwrap();
    assert_eq!(shipped.origin, AssetOrigin::Shipped);
    assert!(shipped.path.ends_with("mappings/junction.md"));
    assert_eq!(
        assets::resolve_mapping("./missing.md", cwd).unwrap_err().to_string(),
        format!("no mapping file at {}", cwd.join("missing.md").display())
    );
    assert_eq!(assets::resolve_mapping("missing", cwd).unwrap_err().to_string(), "no mapping file named missing");
    for path in ["C:\\mappings\\mine.md", "mappings\\mine.md", "c:mine.md", "./mine.md", "/tmp/mine.md"] {
        assert!(assets::is_mapping_path(path), "{path}");
    }
    assert!(!assets::is_mapping_path("open-wearables"));

    let mut store = new_store(cwd);
    let own = cwd.join(".agents/mappings/my-source.md");
    std::fs::write(
        &own,
        "# My source\r\n\r\n```ath-mapping\r\nname: my-source\r\nroute: my_source\r\nwriter_field: maker\r\nvalue_basis: provider\r\n\r\n[sleep]\r\nid_field: night\r\nstart_field: from\r\nend_field: to\r\nasleep: asleep_seconds s s\r\n```\r\n",
    )
    .unwrap();
    let input = cwd.join("nights.json");
    std::fs::write(
        &input,
        r#"{"sleep":[{"night":"n-1","from":"2026-08-09T01:00:00.000Z","to":"2026-08-09T07:00:00.000Z","maker":"invented-maker","asleep":20000,"mood_word":"fine"}]}"#,
    )
    .unwrap();
    let resolved = assets::resolve_mapping("my-source", cwd).unwrap();
    mapped(&mut store, &input, &mapping::load_mapping(&resolved.text).unwrap(), 0);
    let night = &stored(&store)[&key(&["invented-maker", "-", "sleep", "n-1"])];
    assert_eq!(night.data["asleep_seconds"], 20000);
    assert_eq!(night.data["supplied_fields"], serde_json::json!({ "mood_word": "fine" }));
    assert_eq!(night.data["field_basis"], serde_json::json!({ "value": "provider" }));
    assert_eq!(source(&store, night.data["source_id"].as_str().unwrap())["data_route"], "my_source");
}

/// `garmin_file_and_mapping_stay_two_sources`.
#[test]
fn a_garmin_file_and_a_mapping_stay_two_sources() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let preview = import::preview_shared(
        &store,
        &exports().join("garmin/park-loop.json"),
        import::SharedImport::Garmin,
        &options(&step_time(0)),
    )
    .unwrap();
    preview.apply(&mut store).unwrap();
    mapped(&mut store, &exports().join("open-wearables/summaries.json"), &shipped("open-wearables"), 1);
    let routes: Vec<(String, String)> = store
        .connection()
        .prepare("SELECT data_route, COALESCE(sensor_type, '') FROM sources WHERE data_writer = 'garmin' ORDER BY data_route")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        routes,
        [("garmin_account_export".to_string(), String::new()), ("open_wearables".to_string(), S_WATCH.to_string())]
    );
    let peaks: Vec<f64> = store
        .connection()
        .prepare("SELECT standard_value FROM provider_summaries WHERE summary_type = 'max_heart_rate' ORDER BY standard_value")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(peaks, [180.0, 200.0]);
}

/// The two shipped mapping files parse into typed settings and pass every check.
#[test]
fn the_shipped_mapping_files_load() {
    for name in ["open-wearables", "junction"] {
        let mapping = shipped(name);
        assert_eq!(mapping.name, name);
        assert!(!mapping.sections.is_empty());
    }
    assert!(assets::shipped_mapping("junction")
        .unwrap()
        .contains("average_hrv: hrv_rmssd ms ms basis hrv_mean_rmssd when-writer oura,whoop_v2,garmin,fitbit,polar"));
}

/// A mapped input over the limit is refused before it is read.
#[test]
fn a_mapped_input_over_the_limit_is_refused() {
    let directory = TempDir::new();
    let store = new_store(directory.path());
    let input = directory.path().join("big.json");
    std::fs::File::create(&input).unwrap().set_len(athleticstandard::input_policy::MAX_MAPPED_INPUT_BYTES + 1).unwrap();
    let error =
        import::preview_mapped(&store, &input, &shipped("open-wearables"), &options(&step_time(0))).unwrap_err();
    assert_eq!(error.to_string(), format!("the input exceeds 536870912 bytes: {}", input.display()));
}
