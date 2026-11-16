//! Range receipt version 2 (D244) on invented records. The expected numbers
//! were worked out by hand from the stated formulas.

mod calculation_support;

use athleticstandard::calculate::{self, CalculateOptions, Operation};
use calculation_support::{canon, Lab};
use serde_json::json;

fn receipt(lab: &Lab, options: &CalculateOptions) -> serde_json::Value {
    let receipt = calculate::calculate_range(&lab.store, options).unwrap();
    serde_json::from_str(&athleticstandard::json::encode(&receipt)).unwrap()
}

/// A reading: a measurement sample for a series quantity, else a point measurement.
fn sample(id: &str, source: &str, quantity: &str, at: &str, value: f64) -> serde_json::Value {
    let kind = if athleticstandard::model::series_quantity_unit(quantity).is_some() {
        "measurement_sample"
    } else {
        "point_measurement"
    };
    json!({ "id": id, "record_type": kind, "source": source, "quantity": quantity, "at": at, "value": value })
}

macro_rules! assert_json {
    ($actual:expr, $expected:expr $(, $($message:tt)*)?) => {
        assert_eq!(canon(&$actual), canon(&serde_json::json!($expected)) $(, $($message)*)?)
    };
}

#[test]
fn a_baseline_adds_a_daily_mean_summary_when_it_knows_the_day() {
    let mut lab = Lab::new();
    lab.write(&[
        sample("a", "a", "hrv_sdnn", "2026-10-01T06:00:00.000Z", 40.0),
        sample("b", "a", "hrv_sdnn", "2026-10-01T18:00:00.000Z", 60.0),
        sample("c", "a", "hrv_sdnn", "2026-10-02T06:00:00.000Z", 50.0),
        sample("d", "a", "hrv_sdnn", "2026-10-03T06:00:00.000Z", 80.0),
        sample("e", "a", "hrv_sdnn", "2026-10-03T12:00:00.000Z", 80.0),
        sample("f", "a", "hrv_sdnn", "2026-10-03T18:00:00.000Z", 80.0),
    ]);
    let mut options = CalculateOptions::new(Operation::Baseline, "hrv_sdnn");
    let without_zone = receipt(&lab, &options);
    assert_eq!(without_zone["calculation_version"], "ath-runtime-v2");
    // Every reading counts once: 390 / 6 = 65.
    assert_json!(without_zone["result"][0]["mean"], 65.0);
    assert!(without_zone["result"][0].get("daily").is_none());
    assert!(without_zone["exclusions"][1].as_str().unwrap().starts_with("no daily-mean summary"));
    options.time_zone = Some("UTC".into());
    let with_zone = receipt(&lab, &options);
    // Day means 50, 50, 80: mean 60, population deviation √200 = 14.1421.
    assert_json!(with_zone["result"][0]["mean"], 65.0, "the record-weighted fields keep their meaning");
    assert_json!(
        with_zone["result"][0]["daily"],
        json!({ "time_zone": "UTC", "days": 3, "mean": 60.0, "standard_deviation": 14.1421 })
    );
}

#[test]
fn a_trend_reports_its_span_and_slope_error() {
    let mut lab = Lab::new();
    for (day, value) in [(1, 10.0), (2, 12.0), (3, 13.0), (4, 17.0)] {
        lab.write_one(&sample(
            &format!("t{day}"),
            "a",
            "resting_heart_rate",
            &format!("2026-10-{day:02}T07:00:00.000Z"),
            value,
        ));
    }
    let output = receipt(&lab, &CalculateOptions::new(Operation::Trend, "resting_heart_rate"));
    // Sxy 11, Sxx 5: slope 2.2. Residual squares 1.8 over 2 degrees: √(0.9 / 5) = 0.4243.
    assert_json!(output["result"][0]["slope_per_day"], 2.2);
    assert_json!(output["result"][0]["days_spanned"], 3.0);
    assert_json!(output["result"][0]["slope_standard_error"], 0.4243);
}

#[test]
fn coverage_reports_its_share_of_days() {
    let mut lab = Lab::new();
    for day in [1, 2, 4] {
        lab.write_one(&sample(&format!("c{day}"), "a", "hrv_sdnn", &format!("2026-10-{day:02}T07:00:00.000Z"), 50.0));
    }
    let mut options = CalculateOptions::new(Operation::Coverage, "hrv_sdnn");
    options.time_zone = Some("UTC".into());
    options.from = Some("2026-10-01".into());
    options.to = Some("2026-10-04".into());
    let output = receipt(&lab, &options);
    assert_json!(output["result"][0]["coverage_ratio"], 0.75);
    assert_json!(output["result"][0]["missing_dates"], ["2026-10-03"]);
}

#[test]
fn source_comparison_adds_the_days_both_sources_read() {
    let mut lab = Lab::new();
    lab.write(&[
        sample("a1", "a", "resting_heart_rate", "2026-10-01T07:00:00.000Z", 50.0),
        sample("a2", "a", "resting_heart_rate", "2026-10-02T07:00:00.000Z", 60.0),
        sample("a3", "a", "resting_heart_rate", "2026-10-03T07:00:00.000Z", 70.0),
        sample("b1", "b", "resting_heart_rate", "2026-10-01T07:00:00.000Z", 45.0),
        sample("b3", "b", "resting_heart_rate", "2026-10-03T07:00:00.000Z", 60.0),
        sample("b4", "b", "resting_heart_rate", "2026-10-04T07:00:00.000Z", 100.0),
    ]);
    let mut options = CalculateOptions::new(Operation::CompareSources, "resting_heart_rate");
    options.source_id = Some(lab.source_id("a"));
    options.compare_source_id = Some(lab.source_id("b"));
    options.time_zone = Some("UTC".into());
    let output = receipt(&lab, &options);
    // Unpaired: 60 − 68.3333. Paired days 1 and 3: (5 + 10) / 2 = 7.5.
    assert_json!(output["result"]["mean_difference"], -8.3333);
    assert_json!(output["result"]["paired"], json!({ "time_zone": "UTC", "days": 2, "mean_difference": 7.5 }));
}

#[test]
fn a_correlation_needs_three_paired_days_and_says_why() {
    let mut lab = Lab::new();
    lab.write(&[
        sample("h1", "a", "hrv_rmssd", "2026-10-01T07:00:00.000Z", 50.0),
        sample("h2", "a", "hrv_rmssd", "2026-10-02T07:00:00.000Z", 60.0),
        sample("r1", "a", "resting_heart_rate", "2026-10-01T07:00:00.000Z", 55.0),
        sample("r2", "a", "resting_heart_rate", "2026-10-02T07:00:00.000Z", 50.0),
    ]);
    let mut options = CalculateOptions::new(Operation::Correlation, "hrv_rmssd");
    options.other_quantity = Some("resting_heart_rate".into());
    options.time_zone = Some("UTC".into());
    let output = receipt(&lab, &options);
    // Two days always gave −1 or 1 in 2.1.1.
    assert_json!(output["result"]["correlation"], json!(null));
    assert_json!(output["result"]["reason"], "2 paired days; a correlation needs at least 3");
}

#[test]
fn workout_drift_counts_the_samples_in_each_half() {
    let mut lab = Lab::new();
    lab.write_one(&json!({ "id": "w", "record_type": "workout_session",
        "start": "2026-10-01T10:00:00.000Z", "end": "2026-10-01T10:10:00.000Z" }));
    for (minute, bpm) in [(1, 120.0), (3, 130.0), (6, 140.0), (8, 150.0), (9, 150.0)] {
        lab.write_one(&sample(
            &format!("s{minute}"),
            "a",
            "heart_rate",
            &format!("2026-10-01T10:{minute:02}:00.000Z"),
            bpm,
        ));
    }
    let mut options = CalculateOptions::new(Operation::WorkoutDrift, "heart_rate");
    options.workout_session_id = Some(lab.id("w"));
    let output = receipt(&lab, &options);
    let source = &output["result"]["sources"][0];
    assert_json!(source["first_half_count"], 2);
    assert_json!(source["second_half_count"], 3);
    assert_json!(source["change"], 21.6667);
}
