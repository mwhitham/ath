//! The 2.1.2 one-record methods (D230–D245) on invented records.
//!
//! Each expected number was worked out by hand or with a separate script
//! from the published formula, not by running this program. The tests marked
//! "audit" are the failures recorded in
//! `build-history/v2.1.2/appendix/skill-audit.md`; each keeps its case.

mod calculation_support;

use calculation_support::{canon, Lab};
use serde_json::json;

/// Compare two JSON values with every number read as a double, because the
/// program prints a whole number such as 7.0 as `7`.
macro_rules! assert_json {
    ($actual:expr, $expected:expr $(, $($message:tt)*)?) => {
        assert_eq!(canon(&$actual), canon(&serde_json::json!($expected)) $(, $($message)*)?)
    };
}

fn reading(id: &str, kind: &str, quantity: &str, at: &str, value: f64) -> serde_json::Value {
    json!({ "id": id, "record_type": kind, "quantity": quantity, "at": at, "value": value })
}

fn change(settings: &[(&str, &str)]) -> String {
    Lab::block("earlier_window_change", settings)
}

// ---- earlier_window_change on measurements ---------------------------------

#[test]
fn audit_a_later_reading_never_enters_the_earlier_mean() {
    let mut lab = Lab::new();
    lab.write(&[
        reading("d1", "point_measurement", "hrv_rmssd", "2026-10-01T07:00:00.000Z", 50.0),
        reading("d2", "point_measurement", "hrv_rmssd", "2026-10-02T07:00:00.000Z", 60.0),
        reading("d3", "point_measurement", "hrv_rmssd", "2026-10-03T07:00:00.000Z", 100.0),
    ]);
    let output = lab.apply_text(&change(&[("minimum_days", "1")]), "d2");
    // 2.1.1 reported −15, because the later 100 entered the mean.
    assert_json!(output["result"]["change"], json!(10.0), "{output:#}");
    assert_json!(output["result"]["comparison"]["days"], json!(1));
    assert!(output["exclusions"].as_array().unwrap().iter().any(|text| text == "1 later readings excluded"));
    assert_json!(output["operation"], "earlier_window_change");
    assert!(output.get("method_notice").is_none());
}

#[test]
fn audit_sdnn_samples_have_a_history_and_stay_apart_from_other_kinds() {
    let mut lab = Lab::new();
    lab.write(&[
        reading("s1", "measurement_sample", "hrv_sdnn", "2026-10-01T07:00:00.000Z", 40.0),
        reading("s2", "measurement_sample", "hrv_sdnn", "2026-10-02T07:00:00.000Z", 50.0),
        reading("s3", "measurement_sample", "hrv_sdnn", "2026-10-03T07:00:00.000Z", 60.0),
        // Same statistic as a point measurement, and RMSSD: neither is compared.
        reading("p1", "point_measurement", "hrv_sdnn", "2026-10-02T07:00:00.000Z", 1000.0),
        reading("r1", "point_measurement", "hrv_rmssd", "2026-10-02T07:00:00.000Z", 900.0),
    ]);
    let output = lab.apply_text(&change(&[("minimum_days", "2")]), "s3");
    let result = &output["result"];
    assert_json!(result["quantity"], "hrv_sdnn");
    assert_json!(result["value"], json!(60.0));
    assert_json!(result["change"], json!(15.0));
    assert_json!(result["change_percent"], json!(33.3));
    // Daily means 40 and 50: sample standard deviation 7.071; 15 / 7.071 = 2.12.
    assert_json!(result["comparison"]["standard_deviation"], json!(7.071));
    assert_json!(result["standardized_change"], json!(2.12));
    assert_json!(result["compared_records"], json!(2));
    assert_json!(output["inputs"]["record_count"], json!(2));
}

#[test]
fn readings_are_averaged_by_day_and_the_day_so_far() {
    let mut lab = Lab::new();
    lab.write(&[
        reading("a", "measurement_sample", "hrv_sdnn", "2026-10-01T06:00:00.000Z", 40.0),
        reading("b", "measurement_sample", "hrv_sdnn", "2026-10-01T18:00:00.000Z", 60.0),
        reading("c", "measurement_sample", "hrv_sdnn", "2026-10-02T06:00:00.000Z", 50.0),
        reading("morning", "measurement_sample", "hrv_sdnn", "2026-10-03T08:00:00.000Z", 60.0),
        reading("noon", "measurement_sample", "hrv_sdnn", "2026-10-03T12:00:00.000Z", 80.0),
    ]);
    let settings = change(&[("minimum_days", "2")]);
    // Earlier daily means 50 and 50. At noon the day so far is (60 + 80) / 2 = 70.
    let noon = lab.apply_text(&settings, "noon");
    assert_json!(noon["result"]["value"], json!(70.0));
    assert_json!(noon["result"]["target_day_readings"], json!(2));
    assert_json!(noon["result"]["change"], json!(20.0));
    assert_json!(noon["result"]["comparison"]["standard_deviation"], json!(0.0));
    assert_json!(noon["result"]["standardized_change"], json!(null), "no spread, no standardized change");
    assert_json!(noon["result"]["compared_records"], json!(3));
    // In the morning the noon reading has not happened yet.
    let morning = lab.apply_text(&settings, "morning");
    assert_json!(morning["result"]["value"], json!(60.0));
    assert_json!(morning["result"]["target_day_readings"], json!(1));
}

#[test]
fn a_change_keeps_bases_apart_and_needs_enough_days() {
    let mut lab = Lab::new();
    let mut records = Vec::new();
    for day in 1..=7 {
        records.push(reading(
            &format!("p{day}"),
            "point_measurement",
            "hrv_rmssd",
            &format!("2026-10-{day:02}T07:00:00.000Z"),
            50.0,
        ));
    }
    let mut computed = reading("calc", "point_measurement", "hrv_rmssd", "2026-10-07T09:00:00.000Z", 999.0);
    computed["basis"] = json!("calculated_on_import");
    records.push(computed);
    records.push(reading("old", "point_measurement", "hrv_rmssd", "2026-08-01T07:00:00.000Z", 10.0));
    records.push(reading("target", "point_measurement", "hrv_rmssd", "2026-10-08T07:00:00.000Z", 55.0));
    lab.write(&records);
    let output = lab.apply_text(&change(&[]), "target");
    assert_json!(output["result"]["comparison"]["days"], json!(7), "{output:#}");
    assert_json!(output["result"]["comparison"]["mean"], json!(50.0));
    assert_json!(output["result"]["basis"], "unstated");
    assert!(output["exclusions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|text| text == "1 readings with a basis other than unstated excluded"));
    assert_json!(output["result"]["comparison"]["from"], "2026-09-10");
    assert_json!(output["result"]["comparison"]["to"], "2026-10-07");

    let short = lab.apply_text(&change(&[]), "p3");
    assert_json!(short["result"], json!(null));
    assert_json!(
        short["missing"],
        json!(["found 2 earlier days of hrv_rmssd in the 28 days before 2026-10-03, need 7"])
    );
}

#[test]
fn days_follow_the_athlete_time_zone() {
    let mut lab = Lab::new();
    lab.write(&[
        json!({ "id": "me", "record_type": "athlete_profile", "default_time_zone": "America/Chicago" }),
        // 03:00 UTC on 2 October is the evening of 1 October in Chicago.
        reading("late", "point_measurement", "resting_heart_rate", "2026-10-02T03:00:00.000Z", 50.0),
        reading("day2", "point_measurement", "resting_heart_rate", "2026-10-02T13:00:00.000Z", 52.0),
        reading("day3", "point_measurement", "resting_heart_rate", "2026-10-03T13:00:00.000Z", 56.0),
    ]);
    let output = lab.apply_text(&change(&[("minimum_days", "2")]), "day3");
    assert_json!(output["result"]["time_zone"], "America/Chicago");
    assert_json!(output["result"]["comparison"]["days"], json!(2), "{output:#}");
    assert_json!(output["result"]["change"], json!(5.0));
}

// ---- earlier_window_change on sleep ------------------------------------------

#[test]
fn sleep_change_compares_main_sleeps_and_leaves_out_naps() {
    let mut lab = Lab::new();
    let mut records = Vec::new();
    for night in 1..=7 {
        records.push(json!({
            "id": format!("n{night}"), "record_type": "sleep_session",
            "start": format!("2026-10-{night:02}T23:00:00.000Z"), "end": format!("2026-10-{:02}T07:00:00.000Z", night + 1),
            "asleep_seconds": 28800
        }));
    }
    records.push(json!({ "id": "nap", "record_type": "sleep_session",
        "start": "2026-10-08T13:00:00.000Z", "end": "2026-10-08T14:00:00.000Z", "asleep_seconds": 3600 }));
    records.push(json!({ "id": "target", "record_type": "sleep_session",
        "start": "2026-10-08T23:00:00.000Z", "end": "2026-10-09T07:00:00.000Z", "asleep_seconds": 21600 }));
    records.push(json!({ "id": "later", "record_type": "sleep_session",
        "start": "2026-10-09T23:00:00.000Z", "end": "2026-10-10T07:00:00.000Z", "asleep_seconds": 1000 }));
    records.push(json!({ "id": "overlap", "record_type": "sleep_session",
        "start": "2026-10-08T19:00:00.000Z", "end": "2026-10-08T23:30:00.000Z", "asleep_seconds": 1000 }));
    lab.write(&records);
    let output = lab.apply_text(&change(&[("field", "asleep_seconds")]), "target");
    let result = &output["result"];
    assert_json!(result["field"], "asleep_seconds");
    assert_json!(result["change"], json!(-7200.0), "{output:#}");
    assert_json!(result["change_percent"], json!(-25.0));
    assert_json!(result["comparison"]["nights"], json!(7));
    let exclusions = output["exclusions"].as_array().unwrap();
    assert!(exclusions.iter().any(|text| text == "1 naps shorter than 3 h excluded"), "{exclusions:?}");
    assert!(exclusions.iter().any(|text| text == "1 sessions that overlap this one excluded"), "{exclusions:?}");
    let nap = lab.apply_text(&change(&[("field", "asleep_seconds")]), "nap");
    assert_json!(nap["missing"], json!(["a main sleep of at least 3 h; this session spans 1.0 h"]));
}

// ---- drop_after_effort_end -----------------------------------------------------

#[test]
fn audit_recovery_is_measured_from_the_end_of_effort() {
    let mut lab = Lab::new();
    let samples: Vec<serde_json::Value> = [(590_000, 170), (600_000, 172), (630_000, 155), (660_000, 140)]
        .iter()
        .map(|(offset, bpm)| json!({ "offset_ms": offset, "bpm": bpm }))
        .collect();
    lab.write(&[
        json!({ "id": "kept", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z",
            "end": "2026-10-01T10:10:00.000Z", "heart_rate_samples": samples }),
        json!({ "id": "stopped", "record_type": "workout_session", "start": "2026-10-02T10:00:00.000Z",
            "end": "2026-10-02T10:10:00.000Z", "heart_rate_samples": [
                { "offset_ms": 590_000, "bpm": 170 }, { "offset_ms": 600_000, "bpm": 172 }] }),
        json!({ "id": "lap", "record_type": "workout_session", "start": "2026-10-03T10:00:00.000Z",
            "end": "2026-10-03T10:10:00.000Z",
            "segments": [{ "label": "Recovery", "start_offset_ms": 300_000, "source_reported_summaries": [] }],
            "heart_rate_samples": [{ "offset_ms": 300_000, "bpm": 180 }, { "offset_ms": 355_000, "bpm": 152 },
                { "offset_ms": 365_000, "bpm": 148 }] }),
    ]);
    let block = Lab::block("drop_after_effort_end", &[]);
    let kept = lab.apply_text(&block, "kept");
    assert_json!(
        kept["result"],
        json!({ "drop_bpm": 32.0, "end_bpm": 172.0, "later_bpm": 140.0,
        "end_offset_ms": 600_000, "later_offset_ms": 660_000, "anchor": "workout end" })
    );
    let stopped = lab.apply_text(&block, "stopped");
    assert_json!(stopped["result"], json!(null));
    assert!(stopped["missing"][0].as_str().unwrap().starts_with("heart-rate samples 60 s after the end of effort"));
    // The recovery lap starts at 300 s. 360 s lies between 152 and 148: 150.
    let lap = lab.apply_text(&block, "lap");
    assert_json!(lap["result"]["drop_bpm"], json!(30.0), "{lap:#}");
    assert_json!(lap["result"]["anchor"], "recovery segment");
}

// ---- sleep_regularity_index -----------------------------------------------------------

fn nights(lab: &mut Lab, shifted: bool) {
    let mut records = Vec::new();
    for night in 0..8u32 {
        let day = 1 + night;
        let (start, end) = if shifted && night % 2 == 1 {
            (format!("2026-10-{:02}T03:00:00.000Z", day + 1), format!("2026-10-{:02}T11:00:00.000Z", day + 1))
        } else {
            (format!("2026-10-{day:02}T23:00:00.000Z"), format!("2026-10-{:02}T07:00:00.000Z", day + 1))
        };
        records.push(json!({ "id": format!("n{night}"), "record_type": "sleep_session", "start": start, "end": end }));
    }
    lab.write(&records);
}

#[test]
fn audit_identical_nights_are_perfectly_regular() {
    let mut lab = Lab::new();
    nights(&mut lab, false);
    let output = lab.apply_text(&Lab::block("sleep_regularity_index", &[]), "n7");
    // 2.1.1 scored seven identical nights 0.5.
    assert_json!(output["result"]["index"], json!(100.0), "{output:#}");
    assert_json!(output["result"]["day_pairs"], json!(7));
    assert_json!(output["result"]["days_observed"], json!(8));
}

#[test]
fn an_alternating_schedule_scores_lower() {
    let mut lab = Lab::new();
    nights(&mut lab, true);
    // Worked out minute by minute in a separate script: 33.3.
    let output = lab.apply_text(&Lab::block("sleep_regularity_index", &[]), "n7");
    assert_json!(output["result"]["index"], json!(33.3), "{output:#}");
    let few = lab.apply_text(&Lab::block("sleep_regularity_index", &[]), "n3");
    assert_json!(few["result"], json!(null));
    assert!(few["missing"][0].as_str().unwrap().starts_with("found 3 pairs of consecutive observed days"));
}

// ---- volume_load ----------------------------------------------------------------------------

#[test]
fn audit_pounds_are_converted_before_they_are_added() {
    let mut lab = Lab::new();
    lab.write(&[
        json!({ "id": "mixed", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z",
            "end": "2026-10-01T11:00:00.000Z", "sets": [
                { "reps": 10, "weight": 100, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
                { "reps": 10, "weight": 100, "weight_unit": "lb", "movement": "back squat", "load_basis": "total" },
                { "reps": 10, "weight": 22.5, "weight_unit": "kg", "movement": "dumbbell row",
                  "load_basis": "per_implement", "implement_count": 2 },
                { "reps": 5, "weight": 20, "weight_unit": "kg" }] }),
        json!({ "id": "stone", "record_type": "workout_session", "start": "2026-10-02T10:00:00.000Z",
            "end": "2026-10-02T11:00:00.000Z", "sets": [{ "reps": 5, "weight": 10, "weight_unit": "stone" }] }),
    ]);
    let block = Lab::block("volume_load", &[]);
    let output = lab.apply_text(&block, "mixed");
    // 1000 + 453.59237 + 450 + 100 = 2003.59237. 2.1.1 reported 2000 "kg" for the first two alone.
    assert_json!(output["result"]["volume"], json!(2003.592), "{output:#}");
    assert_json!(output["result"]["weight_unit"], "kg");
    assert_json!(output["result"]["unstated_load_basis_sets"], json!(1));
    let movements = output["result"]["by_movement"].as_array().unwrap();
    assert_json!(movements[0], json!({ "movement": "back squat", "volume": 1453.592, "sets": 2, "reps": 20.0 }));
    assert_json!(movements[1]["volume"], json!(450.0));
    assert_json!(movements[2]["movement"], json!(null));
    let stone = lab.apply_text(&block, "stone");
    assert_json!(stone["missing"], json!(["set 1 weight_unit 'stone' is not kg or lb"]));
}

// ---- session_effort_load ----------------------------------------------------------------------

#[test]
fn audit_only_a_session_effort_becomes_training_load() {
    let mut lab = Lab::new();
    lab.write(&[
        json!({ "id": "workout", "record_type": "workout_session", "start": "2026-10-01T17:00:00.000Z",
            "end": "2026-10-01T17:45:00.000Z" }),
        json!({ "id": "effort", "record_type": "athlete_report", "report_type": "session_effort",
            "at": "2026-10-01T18:15:00.000Z", "rating": 7, "rating_scale": "0-10", "duration_seconds": 3600 }),
        json!({ "id": "borg", "record_type": "athlete_report", "report_type": "session_effort",
            "at": "2026-10-01T18:15:00.000Z", "rating": 15, "rating_scale": "6-20", "workout": "workout" }),
        json!({ "id": "sore", "record_type": "athlete_report", "report_type": "soreness",
            "at": "2026-10-01T18:15:00.000Z", "rating": 4, "rating_scale": "1-5", "duration_seconds": 3600 }),
        json!({ "id": "odd", "record_type": "athlete_report", "report_type": "session_effort",
            "at": "2026-10-01T18:15:00.000Z", "rating": 4, "rating_scale": "1-5", "duration_seconds": 3600 }),
        json!({ "id": "high", "record_type": "athlete_report", "report_type": "session_effort",
            "at": "2026-10-01T18:15:00.000Z", "rating": 11, "rating_scale": "0-10", "duration_seconds": 3600 }),
    ]);
    let block = Lab::block("session_effort_load", &[]);
    let effort = lab.apply_text(&block, "effort");
    assert_json!(
        effort["result"],
        json!({ "load": 420.0, "unit": "AU", "rating": 7.0, "rating_scale": "0-10",
        "scale_family": "10-point", "duration_minutes": 60.0, "duration_from": "report" })
    );
    // No duration on the report: the linked 45-minute workout. 15 × 45 = 675.
    let borg = lab.apply_text(&block, "borg");
    assert_json!(borg["result"]["load"], json!(675.0), "{borg:#}");
    assert_json!(borg["result"]["scale_family"], "6-20");
    assert_json!(borg["result"]["duration_from"], "linked workout session");
    assert_json!(borg["inputs"]["record_count"], json!(1));
    // 2.1.1 turned this soreness rating into a load of 240.
    let sore = lab.apply_text(&block, "sore");
    assert_json!(sore["result"], json!(null));
    assert!(sore["missing"][0].as_str().unwrap().contains("this is a soreness athlete report"));
    assert!(lab.apply_text(&block, "odd")["missing"][0].as_str().unwrap().contains("found '1-5'"));
    assert!(lab.apply_text(&block, "high")["missing"][0].as_str().unwrap().contains("found 11"));
}

// ---- banister_trimp and time_in_zones --------------------------------------------------------

#[test]
fn trimp_uses_the_athletes_weighting_and_covered_time() {
    let samples = json!([{ "offset_ms": 0, "bpm": 130 }, { "offset_ms": 60_000, "bpm": 172 }]);
    let workout = json!({ "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z",
        "end": "2026-10-01T10:02:00.000Z", "resting_heart_rate_bpm": 60, "maximum_heart_rate_bpm": 200,
        "heart_rate_samples": samples });
    let block = Lab::block("banister_trimp", &[]);
    // Reserve 0.5 then 0.8 for one minute each.
    // Men: 0.5 × 0.64 e^0.96 + 0.8 × 0.64 e^1.536 = 3.214. Women: 0.86 and 1.67 give 3.608.
    for (sex, load, coefficient) in [("male", 3.2, 0.64), ("female", 3.6, 0.86)] {
        let mut lab = Lab::new();
        lab.write(&[json!({ "id": "me", "record_type": "athlete_profile", "sex": sex }), workout.clone()]);
        let output = lab.apply_text(&block, "w");
        assert_json!(output["result"]["load"], json!(load), "{output:#}");
        assert_json!(output["result"]["coefficient"], json!(coefficient));
        assert_json!(output["result"]["covered_minutes"], json!(2.0));
    }
    let mut lab = Lab::new();
    lab.write(&[workout]);
    let unknown = lab.apply_text(&block, "w");
    assert_json!(unknown["result"], json!(null));
    assert!(unknown["missing"][0].as_str().unwrap().starts_with("athlete_profile.sex"));
}

#[test]
fn zones_credit_only_covered_time() {
    let mut lab = Lab::new();
    lab.write(&[
        json!({ "id": "steady", "record_type": "workout_session", "start": "2026-10-01T12:00:00.000Z",
            "end": "2026-10-01T12:00:04.000Z", "maximum_heart_rate_bpm": 200, "heart_rate_samples": [
                { "offset_ms": 0, "bpm": 100 }, { "offset_ms": 1000, "bpm": 130 },
                { "offset_ms": 2000, "bpm": 160 }, { "offset_ms": 3000, "bpm": 190 }] }),
        json!({ "id": "gap", "record_type": "workout_session", "start": "2026-10-02T12:00:00.000Z",
            "end": "2026-10-02T12:03:25.000Z", "maximum_heart_rate_bpm": 200, "heart_rate_samples": [
                { "offset_ms": 0, "bpm": 100 }, { "offset_ms": 200_000, "bpm": 190 }] }),
    ]);
    let block = Lab::block("time_in_zones", &[]);
    let steady = lab.apply_text(&block, "steady");
    assert_json!(steady["result"]["seconds"], json!([1.0, 1.0, 0.0, 1.0, 1.0]), "{steady:#}");
    assert_json!(steady["result"]["boundaries_bpm"], json!([120.0, 140.0, 160.0, 180.0]));
    // 200 s between samples: 60 s credited, 140 s uncovered; the last sample lasts 5 s to the end.
    let gap = lab.apply_text(&block, "gap");
    assert_json!(gap["result"]["seconds"], json!([60.0, 0.0, 0.0, 0.0, 5.0]), "{gap:#}");
    assert_json!(gap["result"]["uncovered_seconds"], json!(140.0));
}

// ---- lowest_rolling_mean -----------------------------------------------------------------------

#[test]
fn audit_resting_heart_rate_is_a_sustained_low_not_a_flat_line() {
    let mut lab = Lab::new();
    let samples: Vec<serde_json::Value> = (0..=40)
        .map(|step| {
            let offset = step * 30_000;
            let bpm = if (360_000..=660_000).contains(&offset) { 52 } else { 60 + step % 3 };
            json!({ "offset_ms": offset, "bpm": bpm })
        })
        .collect();
    let sparse: Vec<serde_json::Value> = (0..4).map(|step| json!({ "offset_ms": step * 600_000, "bpm": 50 })).collect();
    lab.write(&[
        json!({ "id": "w", "record_type": "workout_session", "start": "2026-10-01T10:00:00.000Z",
            "end": "2026-10-01T10:20:00.000Z", "heart_rate_samples": samples }),
        json!({ "id": "sparse", "record_type": "workout_session", "start": "2026-10-02T10:00:00.000Z",
            "end": "2026-10-02T10:40:00.000Z", "heart_rate_samples": sparse }),
    ]);
    let block = Lab::block("lowest_rolling_mean", &[]);
    let output = lab.apply_text(&block, "w");
    assert_json!(output["result"]["bpm"], json!(52.0), "{output:#}");
    assert_json!(output["result"]["window_start_offset_ms"], json!(360_000));
    assert_json!(output["result"]["samples_in_window"], json!(11));
    let sparse = lab.apply_text(&block, "sparse");
    assert_json!(sparse["result"], json!(null));
}

#[test]
fn sleep_resting_heart_rate_reads_samples_from_the_same_source() {
    let mut lab = Lab::new();
    let mut records = vec![json!({ "id": "night", "record_type": "sleep_session",
        "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z" })];
    for (minute, bpm) in [58, 57, 56, 50, 50, 50, 50, 50, 50, 55].iter().enumerate() {
        records.push(reading(
            &format!("hr{minute}"),
            "measurement_sample",
            "heart_rate",
            &format!("2026-10-02T02:{minute:02}:00.000Z"),
            f64::from(*bpm),
        ));
    }
    let mut other = reading("other", "measurement_sample", "heart_rate", "2026-10-02T03:00:00.000Z", 30.0);
    other["source"] = json!("b");
    records.push(other);
    records.push(reading("outside", "measurement_sample", "heart_rate", "2026-10-02T09:00:00.000Z", 30.0));
    lab.write(&records);
    let output = lab.apply_text(&Lab::block("lowest_rolling_mean", &[]), "night");
    assert_json!(output["result"]["bpm"], json!(50.0), "{output:#}");
    assert_json!(output["result"]["window_start_at"], "2026-10-02T02:03:00.000Z");
    assert_json!(output["result"]["samples_in_window"], json!(6));
    assert_json!(output["result"]["samples_read"], json!(10));
}

// ---- sleep efficiency, stages, and clock times ----------------------------------------------------

#[test]
fn sleep_efficiency_stages_and_clock_times() {
    let mut lab = Lab::new();
    lab.write(&[
        json!({ "id": "me", "record_type": "athlete_profile", "default_time_zone": "America/Chicago" }),
        json!({ "id": "night", "record_type": "sleep_session", "start": "2026-10-03T04:30:00.000Z",
            "end": "2026-10-03T12:30:00.000Z", "asleep_seconds": 27000, "in_bed_seconds": 28800,
            "stage_minutes": { "deep": 63, "rem": 90, "light": 197, "awake": 30 } }),
        json!({ "id": "odd", "record_type": "sleep_session", "start": "2026-10-04T04:30:00.000Z",
            "end": "2026-10-04T12:30:00.000Z", "asleep_seconds": 30000, "in_bed_seconds": 28800 }),
    ]);
    let efficiency = lab.apply_text(&Lab::block("sleep_efficiency", &[]), "night");
    assert_json!(
        efficiency["result"],
        json!({ "efficiency": 0.9375, "percent": 93.8,
        "asleep_seconds": 27000.0, "in_bed_seconds": 28800.0 })
    );
    let odd = lab.apply_text(&Lab::block("sleep_efficiency", &[]), "odd");
    assert_json!(odd["missing"], json!(["asleep_seconds is more than in_bed_seconds, so the record is inconsistent"]));
    let stages = lab.apply_text(&Lab::block("sleep_stage_split", &[]), "night");
    assert_json!(stages["result"]["asleep_minutes"], json!(350.0), "{stages:#}");
    assert_json!(stages["result"]["awake_minutes"], json!(30.0));
    assert_json!(stages["result"]["percent_of_asleep"], json!({ "deep": 18.0, "rem": 25.7, "light": 56.3 }));
    let times = lab.apply_text(&Lab::block("sleep_clock_times", &[]), "night");
    assert_json!(times["result"]["local_start"], "2026-10-02T23:30:00-05:00", "{times:#}");
    assert_json!(times["result"]["local_midpoint"], "2026-10-03T03:30:00-05:00");
    assert_json!(times["result"]["local_end"], "2026-10-03T07:30:00-05:00");
    assert_json!(times["result"]["time_zone_basis"], "athlete_profile.default_time_zone");
    assert_json!(times["result"]["duration_hours"], json!(8.0));
}

// ---- hyrox_splits -------------------------------------------------------------------------------

fn hyrox(lab: &mut Lab, id: &str, roxzone: serde_json::Value, total: f64) {
    let mut segments: Vec<serde_json::Value> =
        (1..=8).map(|run| json!({ "label": format!("run_{run}"), "score": { "duration_s": 280 + run * 5 } })).collect();
    for (station, seconds) in [
        ("ski_erg", 270),
        ("sled_push", 180),
        ("sled_pull", 240),
        ("burpee_broad_jumps", 270),
        ("row", 280),
        ("farmers_carry", 120),
        ("sandbag_lunges", 260),
        ("wall_balls", 330),
    ] {
        segments.push(json!({ "label": station, "score": { "duration_s": seconds } }));
    }
    if let serde_json::Value::Array(extra) = roxzone {
        segments.extend(extra);
    }
    lab.write_one(&json!({ "id": id, "record_type": "benchmark_result", "benchmark": "hyrox-race",
        "benchmark_type": "hyrox", "at": "2026-11-01T10:00:00.000Z", "score": { "duration_s": total },
        "segments": segments,
        "extensions": { "org.athleticstandard/hyrox-race": {
            "season": "2026/27", "format": "singles", "division": "open", "category": "women", "penalties_s": 0 } } }));
}

#[test]
fn hyrox_splits_add_up_without_counting_time_twice() {
    let mut lab = Lab::new();
    // Runs 285 to 320: 2420 s. Stations: 1950 s. Roxzone: 420 s. Total 4800: 10 s unaccounted.
    hyrox(&mut lab, "race", json!([{ "label": "roxzone", "score": { "duration_s": 420 } }]), 4800.0);
    hyrox(
        &mut lab,
        "twice",
        json!([{ "label": "roxzone", "score": { "duration_s": 420 } },
        { "label": "roxzone_1", "score": { "duration_s": 30 } }]),
        4800.0,
    );
    hyrox(&mut lab, "over", json!([{ "label": "roxzone", "score": { "duration_s": 900 } }]), 4800.0);
    let block = Lab::block("hyrox_splits", &[]);
    let output = lab.apply_text(&block, "race");
    let result = &output["result"];
    assert_json!(result["runs_seconds"], json!(2420.0), "{output:#}");
    assert_json!(result["stations_seconds"], json!(1950.0));
    assert_json!(result["roxzone_seconds"], json!(420.0));
    assert_json!(result["unaccounted_seconds"], json!(10.0));
    assert_json!(result["share_percent"]["runs"], json!(50.4));
    assert_json!(result["run_summary"]["fastest"], json!({ "label": "run_1", "seconds": 285.0 }));
    assert_json!(result["run_summary"]["slowest"], json!({ "label": "run_8", "seconds": 320.0 }));
    assert_json!(result["run_summary"]["first_run_seconds"], json!(285.0));
    assert_json!(result["run_summary"]["later_runs_mean_seconds"], json!(305.0));
    assert_json!(result["stations"][7], json!({ "label": "wall_balls", "seconds": 330.0, "share_percent": 6.9 }));
    assert_json!(result["season"], "2026/27");
    let twice = lab.apply_text(&block, "twice");
    assert!(twice["missing"][0].as_str().unwrap().contains("both would count Roxzone twice"), "{twice:#}");
    let over = lab.apply_text(&block, "over");
    assert!(over["missing"][0].as_str().unwrap().contains("so some time is counted twice"), "{over:#}");
}

// ---- refusals ----------------------------------------------------------------------------------

#[test]
fn audit_apply_names_the_command_for_a_range_calculation() {
    let mut lab = Lab::new();
    lab.write(&[reading("p", "point_measurement", "hrv_rmssd", "2026-10-01T07:00:00.000Z", 50.0)]);
    let text = "```ath-calculation\nname: baseline\nneeds: quantity\nuse: mean\n```\n";
    let output = lab.apply_text(text, "p");
    assert_json!(
        output["error"],
        "baseline is a range calculation over many records, not one record. Run: ath calculate baseline <quantity>"
    );
}
