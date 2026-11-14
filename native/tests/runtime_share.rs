//! `ath share` stays an explicit placeholder that names the latest report.

use std::fs;
use std::time::{Duration, SystemTime};

use athleticstandard::runtime::share::{latest_report, share_refusal, share_status};

fn touch(path: &std::path::Path, seconds_ago: u64) {
    fs::write(path, "{}\n").unwrap();
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds_ago)).unwrap();
}

#[test]
fn without_a_report_it_says_to_run_a_backtest() {
    let dir = tempfile::tempdir().unwrap();
    let status = share_status(&dir.path().join("athlete.ath.sqlite"), dir.path()).unwrap();
    assert_eq!(serde_json::to_string(&status).unwrap(), r#"{"built":false,"latest_report":null}"#);
    assert_eq!(
        share_refusal(None),
        "ath share is not built yet.\nRun `ath backtest` first. The shareable part of a report is its summary."
    );
}

#[test]
fn names_the_newest_report_beside_the_database_or_in_the_working_directory() {
    let data = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let database = data.path().join("athlete.ath.sqlite");
    touch(&data.path().join("backtest-2026-01-01T000000.json"), 300);
    touch(&cwd.path().join("backtest-2026-01-02T000000.json"), 100);
    touch(&data.path().join("backtest-2026-01-03T000000.json"), 200);
    touch(&data.path().join("notes.json"), 0);

    let newest = cwd.path().join("backtest-2026-01-02T000000.json");
    assert_eq!(latest_report(&database, cwd.path()).unwrap(), Some(newest.clone()));
    let status = share_status(&database, cwd.path()).unwrap();
    assert!(!status.built);
    assert_eq!(
        share_refusal(status.latest_report.as_deref()),
        format!(
            "ath share is not built yet.\nThe shareable part of a backtest is the summary in:\n  {}",
            newest.display()
        )
    );
}

/// 2.1.0 writes reports named from the full timestamp, such as
/// `backtest-2026-01-02T030405678Z.json`, but `share` looks only for
/// `backtest-YYYY-MM-DDTHHMMSS.json`. That mismatch is kept as found.
#[test]
fn the_2_1_0_report_name_mismatch_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("backtest-2026-01-02T030405678Z.json"), 0);
    assert_eq!(latest_report(&dir.path().join("athlete.ath.sqlite"), dir.path()).unwrap(), None);
}
