//! `ath calculate compare-sources`, `correlation`, and `workout-drift`, run
//! through the built executable on the demo database with invented series
//! (`demo_support::write_with_series`). No 2.1.0 test ran these three
//! operations as commands. Every expected number is worked by hand from the
//! series constants, and the complete output is compared.

mod common;
mod demo_support;

use std::path::Path;
use std::process::{Command, Stdio};

use athleticstandard::json::{self, Value};
use common::private_folder;
use demo_support::{series_id, SOURCE_A, SOURCE_B, WORKOUT};
use serde_json::json;

fn ath(cwd: &Path, argv: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success() && stderr.is_empty(), "{argv:?}: {stderr}");
    String::from_utf8(output.stdout).unwrap()
}

/// References to added records, as the receipt lists them.
fn references(indexes: impl IntoIterator<Item = usize>) -> Value {
    Value::Array(indexes.into_iter().map(|index| json!({ "record_id": series_id(index), "revision": 1 })).collect())
}

/// The receipt `--json` prints, and the text a person sees: the summary
/// lines, then the result.
fn outputs(receipt: &Value) -> (String, String) {
    let sources: Vec<&str> = receipt["sources"].as_array().unwrap().iter().map(|id| id.as_str().unwrap()).collect();
    let human = format!(
        "{}: {} {} ({})\nmethod: {}\ninputs: {} records\nsources: {}\n{}\n",
        receipt["operation"].as_str().unwrap(),
        receipt["record_kind"].as_str().unwrap(),
        receipt["quantity"].as_str().unwrap(),
        receipt["unit"].as_str().unwrap(),
        receipt["method"].as_str().unwrap(),
        receipt["input"]["record_count"],
        sources.join(", "),
        json::to_pretty(&receipt["result"]),
    );
    (format!("{}\n", json::to_pretty(receipt)), human)
}

fn check(folder: &Path, argv: &[&str], receipt: &Value) {
    let (expected_json, expected_text) = outputs(receipt);
    let mut with_json = argv.to_vec();
    with_json.push("--json");
    assert_eq!(ath(folder, &with_json), expected_json, "{argv:?} --json");
    assert_eq!(ath(folder, argv), expected_text, "{argv:?}");
}

/// Indexes of added records (`demo_support::series_id`): 0 is source B; each
/// day d adds source A SDNN (1 + 3d), source B SDNN (2 + 3d), and source A
/// respiratory rate (3 + 3d); 16 to 21 are the workout heart-rate samples.
fn a_sdnn() -> impl Iterator<Item = usize> {
    (0..5).map(|day| 1 + 3 * day)
}

#[test]
fn compare_sources_keeps_each_source_and_reports_the_mean_difference() {
    let folder = private_folder();
    demo_support::write_with_series(folder.path());
    // Source A: 40, 42, 44, 46, 48. Source B: each 2 ms lower. Population
    // standard deviation sqrt(8) = 2.8284 for both; mean difference 2.
    let receipt = json!({
        "calculation_version": "ath-runtime-v1",
        "operation": "compare-sources",
        "record_kind": "measurement",
        "method": "separate source summaries and difference between their means",
        "quantity": "hrv_sdnn",
        "unit": "ms",
        "range": {},
        "sources": [SOURCE_A, SOURCE_B],
        "input": {
            "record_count": 10,
            "record_references": references(a_sdnn().chain((0..5).map(|day| 2 + 3 * day))),
            "references_truncated": false
        },
        "exclusions": ["manual sources excluded", "records are not paired by timestamp"],
        "result": {
            "source": {
                "source_id": SOURCE_A, "count": 5, "minimum": 40, "maximum": 48, "mean": 44, "sum": 220,
                "standard_deviation": 2.8284
            },
            "comparison_source": {
                "source_id": SOURCE_B, "count": 5, "minimum": 38, "maximum": 46, "mean": 42, "sum": 210,
                "standard_deviation": 2.8284
            },
            "mean_difference": 2
        }
    });
    let argv = ["calculate", "compare-sources", "hrv_sdnn", "--source", SOURCE_A, "--compare-source", SOURCE_B];
    check(folder.path(), &argv, &receipt);
}

#[test]
fn correlation_pairs_same_day_means() {
    let folder = private_folder();
    demo_support::write_with_series(folder.path());
    // SDNN rises 2 ms a day while respiratory rate falls 0.5 a day on the same
    // five days, so the Pearson correlation is exactly -1.
    let receipt = json!({
        "calculation_version": "ath-runtime-v1",
        "operation": "correlation",
        "record_kind": "measurement",
        "method": "Pearson correlation of same-calendar-day means",
        "quantity": "hrv_sdnn",
        "unit": "ms",
        "range": { "time_zone": "UTC" },
        "sources": [SOURCE_A],
        "input": {
            "record_count": 10,
            "record_references": references(a_sdnn().chain((0..5).map(|day| 3 + 3 * day))),
            "references_truncated": false
        },
        "exclusions": ["manual sources excluded", "days missing either quantity excluded"],
        "result": {
            "other_quantity": "respiratory_rate",
            "paired_day_count": 5,
            "correlation": -1,
            "paired_dates": demo_support::SERIES_DAYS
        }
    });
    let argv = [
        "calculate",
        "correlation",
        "hrv_sdnn",
        "--other-quantity",
        "respiratory_rate",
        "--source",
        SOURCE_A,
        "--timezone",
        "UTC",
    ];
    check(folder.path(), &argv, &receipt);
}

#[test]
fn workout_drift_compares_the_two_halves_of_the_session() {
    let folder = private_folder();
    demo_support::write_with_series(folder.path());
    // The session runs 17:25 to 17:48, so its midpoint is 17:36:30. The first
    // half averages 142 bpm and the second 152: a change of 10, which is
    // 10 / 142 = 7.0423 percent.
    let receipt = json!({
        "calculation_version": "ath-runtime-v1",
        "operation": "workout-drift",
        "record_kind": "measurement",
        "method": "difference between means in equal-duration first and second workout halves",
        "quantity": "heart_rate",
        "unit": "bpm",
        "range": { "from": "2026-08-10T17:25:00.000Z", "to": "2026-08-10T17:48:00.000Z" },
        "sources": [SOURCE_A],
        "input": { "record_count": 6, "record_references": references(16..22), "references_truncated": false },
        "exclusions": ["manual sources excluded", "samples outside the workout session excluded"],
        "result": {
            "workout_session_id": WORKOUT,
            "sources": [{
                "source_id": SOURCE_A,
                "available": true,
                "first_half_mean": 142,
                "second_half_mean": 152,
                "change": 10,
                "change_percentage": 7.0423
            }]
        }
    });
    let argv = ["calculate", "workout-drift", "heart_rate", "--session", WORKOUT];
    check(folder.path(), &argv, &receipt);
}

/// The database file's SHA-256, the WAL's length, and the database revision.
fn database_state(path: &Path) -> (String, u64, u64) {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(std::fs::read(path).unwrap()).iter().map(|byte| format!("{byte:02x}")).collect();
    let wal = athleticstandard::files::sidecar(path, "-wal");
    let wal_length = std::fs::metadata(wal).map(|metadata| metadata.len()).unwrap_or(0);
    (hash, wal_length, common::database_revision(path))
}

#[test]
fn calculations_leave_the_database_unchanged() {
    let folder = private_folder();
    let path = demo_support::write_with_series(folder.path());
    let before = database_state(&path);
    let (first_day, last_day) = (demo_support::SERIES_DAYS[0], demo_support::SERIES_DAYS[4]);
    let commands: [&[&str]; 9] = [
        &["calculate", "summary", "hrv_sdnn", "--source", SOURCE_A],
        &["calculate", "baseline", "hrv_sdnn"],
        &["calculate", "trend", "hrv_sdnn", "--source", SOURCE_A],
        &[
            "calculate",
            "coverage",
            "hrv_sdnn",
            "--source",
            SOURCE_A,
            "--timezone",
            "UTC",
            "--from",
            first_day,
            "--to",
            last_day,
        ],
        &["calculate", "compare-sources", "hrv_sdnn", "--source", SOURCE_A, "--compare-source", SOURCE_B],
        &["calculate", "correlation", "hrv_sdnn", "--other-quantity", "respiratory_rate", "--timezone", "UTC"],
        &["calculate", "workout-drift", "heart_rate", "--session", WORKOUT],
        &[
            "calculate",
            "provider-period-summary",
            "recovery",
            "--record-kind",
            "provider-score",
            "--period-type",
            "whoop_physiological_cycle",
        ],
        &["apply", "heart-rate-zones", "--record", WORKOUT],
    ];
    for argv in commands {
        for json in [false, true] {
            let mut argv = argv.to_vec();
            if json {
                argv.push("--json");
            }
            ath(folder.path(), &argv);
            assert_eq!(database_state(&path), before, "{argv:?}");
        }
    }
}
