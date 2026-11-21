//! The synthetic athletes behind the 2.1.2 agent evaluation (D247).
//!
//! Each scenario is a small invented database. This test builds it, runs the
//! calculations an agent should run, and checks the facts a correct answer must
//! contain. The evaluation in `build-history/v2.1.2/appendix/agent-evaluation.md`
//! grades agent answers against these facts.
//!
//! Set `ATH_EVAL_OUT=<folder>` to keep verified copies of the databases for an
//! agent run. Every value is invented.

mod calculation_support;

use calculation_support::Lab;
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};

fn save(lab: &Lab, name: &str) {
    if let Ok(folder) = std::env::var("ATH_EVAL_OUT") {
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        lab.save_copy(&folder.join(format!("{name}.ath.sqlite")));
    }
}

fn at(day: NaiveDate, hour: u32, minute: u32) -> String {
    Utc.from_utc_datetime(&day.and_hms_opt(hour, minute, 0).unwrap()).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| panic!("not a number: {value}"))
}

/// 35 days of a ring's nightly RMSSD, resting heart rate, and sleep, a watch's
/// SDNN samples with a four-day gap, a week of training with session effort and
/// a soreness report, and a strength session in two units. The last night is
/// short and the last readings are poor.
pub fn recovery() -> Lab {
    let mut lab = Lab::new();
    lab.write_one(
        &json!({ "id": "me", "record_type": "athlete_profile", "default_time_zone": "Europe/London", "sex": "female" }),
    );
    let first = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let last = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    let mut day = first;
    let mut index = 0;
    while day <= last {
        let final_day = day == last;
        let rmssd = if final_day { 46.0 } else { 58.0 + f64::from((index * 7) % 9) - 4.0 };
        let resting = if final_day { 57.0 } else { 52.0 + f64::from((index * 5) % 3) - 1.0 };
        lab.write_one(&json!({ "id": format!("rmssd-{day}"), "record_type": "point_measurement", "source": "ring",
            "quantity": "hrv_rmssd", "at": at(day, 6, 0), "value": rmssd }));
        lab.write_one(&json!({ "id": format!("rhr-{day}"), "record_type": "point_measurement", "source": "ring",
            "quantity": "resting_heart_rate", "at": at(day, 6, 0), "value": resting }));
        // The night that ends on `day`. Friday and Saturday nights run late.
        let night = day - Duration::days(1);
        let weekend = matches!(night.weekday(), chrono::Weekday::Fri | chrono::Weekday::Sat);
        let (start, end, asleep) = if final_day {
            (at(day, 0, 30), at(day, 6, 15), 19_800)
        } else if weekend {
            (at(day, 1, 0), at(day, 9, 30), 28_200 + (index % 3) * 300)
        } else {
            (at(night, 22, 45), at(day, 6, 30), 25_800 + (index % 4) * 300)
        };
        lab.write_one(&json!({ "id": format!("sleep-{day}"), "record_type": "sleep_session", "source": "ring",
            "start": start, "end": end, "asleep_seconds": asleep, "in_bed_seconds": asleep + 1_800 }));
        let watch_off = (NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()
            ..=NaiveDate::from_ymd_opt(2026, 10, 23).unwrap())
            .contains(&day);
        if !watch_off && !final_day {
            for (sample, hour) in [7, 13, 19].iter().enumerate() {
                let value = 45.0 + f64::from((index * 3 + sample as u32 * 5) % 11) - 5.0;
                lab.write_one(&json!({ "id": format!("sdnn-{day}-{hour}"), "record_type": "measurement_sample",
                    "source": "watch", "quantity": "hrv_sdnn", "at": at(day, *hour, 0), "value": value }));
            }
        }
        if final_day {
            lab.write_one(&json!({ "id": "sdnn-today", "record_type": "measurement_sample", "source": "watch",
                "quantity": "hrv_sdnn", "at": at(day, 7, 0), "value": 31.0 }));
        }
        day += Duration::days(1);
        index += 1;
    }
    // The last training week: 28 October to 3 November.
    let sessions = [
        ("2026-10-28", "intervals", 60, 7.0),
        ("2026-10-29", "easy run", 45, 3.0),
        ("2026-10-31", "class", 60, 8.0),
        ("2026-11-01", "long run", 90, 5.0),
        ("2026-11-02", "strength", 60, 6.0),
        ("2026-11-03", "class", 60, 9.0),
    ];
    for (date, kind, minutes, rating) in sessions {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        let mut workout = json!({ "id": format!("workout-{date}"), "record_type": "workout_session", "source": "ring",
            "start": at(day, 17, 0), "end": at(day, 17, 0).replace("17:00", &format!("{}:{:02}", 17 + minutes / 60, minutes % 60)),
            "activity_type": kind, "name": kind });
        if kind == "strength" {
            workout["sets"] = json!([
                { "reps": 5, "weight": 80, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
                { "reps": 5, "weight": 80, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
                { "reps": 5, "weight": 80, "weight_unit": "kg", "movement": "back squat", "load_basis": "total" },
                { "reps": 8, "weight": 135, "weight_unit": "lb", "movement": "romanian deadlift", "load_basis": "total" },
                { "reps": 8, "weight": 135, "weight_unit": "lb", "movement": "romanian deadlift", "load_basis": "total" },
                { "reps": 10, "weight": 16, "weight_unit": "kg", "movement": "dumbbell row", "load_basis": "per_implement", "implement_count": 2 }
            ]);
        }
        lab.write_one(&workout);
        lab.write_one(
            &json!({ "id": format!("effort-{date}"), "record_type": "athlete_report", "report_type": "session_effort",
            "at": at(day, 19, 0), "rating": rating, "rating_scale": "0-10", "duration_seconds": minutes * 60,
            "workout": format!("workout-{date}") }),
        );
    }
    lab.write_one(&json!({ "id": "sore-legs", "record_type": "athlete_report", "report_type": "soreness",
        "at": "2026-11-03T08:00:00.000Z", "rating": 4, "rating_scale": "1-5", "body_region": "quadriceps",
        "duration_seconds": 3600, "note": "legs very sore after the long run and squats" }));
    lab
}

#[test]
fn the_recovery_scenario_has_the_facts_a_correct_answer_needs() {
    let mut lab = recovery();
    // HRV: last night's RMSSD is far below normal for this ring.
    let hrv = lab.apply_shipped("hrv-change", "rmssd-2026-11-04");
    assert_eq!(hrv["result"]["quantity"], "hrv_rmssd", "{hrv:#}");
    assert!(number(&hrv["result"]["standardized_change"]) < -2.0, "{hrv:#}");
    assert_eq!(hrv["result"]["time_zone"], "Europe/London");
    // SDNN from the watch is a separate statistic and a separate device.
    let sdnn = lab.apply_shipped("hrv-change", "sdnn-today");
    assert_eq!(sdnn["result"]["quantity"], "hrv_sdnn", "{sdnn:#}");
    assert!(number(&sdnn["result"]["change"]) < 0.0);
    let resting = lab.apply_shipped("resting-heart-rate-change", "rhr-2026-11-04");
    assert!(number(&resting["result"]["change"]) >= 4.0, "{resting:#}");
    let sleep = lab.apply_shipped("sleep-change", "sleep-2026-11-04");
    assert!(number(&sleep["result"]["change"]) < -5_000.0, "{sleep:#}");
    // Weekend nights run late, so regularity is clearly below 100.
    let regularity = lab.apply_shipped("sleep-regularity", "sleep-2026-11-04");
    let index = number(&regularity["result"]["index"]);
    assert!((20.0..90.0).contains(&index), "{regularity:#}");
    // Effort load: six session efforts; the soreness report is refused.
    let mut week = 0.0;
    for date in ["2026-10-28", "2026-10-29", "2026-10-31", "2026-11-01", "2026-11-02", "2026-11-03"] {
        week += number(&lab.apply_shipped("training-load-effort", &format!("effort-{date}"))["result"]["load"]);
    }
    assert_eq!(week, 420.0 + 135.0 + 480.0 + 450.0 + 360.0 + 540.0);
    assert_eq!(lab.apply_shipped("training-load-effort", "sore-legs")["result"], Value::Null);
    // Strength volume: 1200 kg of squats, 979.760 kg of deadlifts logged in pounds, 320 kg of rows.
    let volume = lab.apply_shipped("strength-volume", "workout-2026-11-02");
    assert_eq!(number(&volume["result"]["volume"]), 2499.76, "{volume:#}");
    // The watch was off for four days.
    let coverage = lab
        .calculate(&json!({ "operation": "coverage", "quantity": "hrv_sdnn", "time_zone": "Europe/London",
        "from": "2026-10-01", "to": "2026-11-04" }));
    assert_eq!(coverage["result"][0]["missing_dates"], json!(["2026-10-20", "2026-10-21", "2026-10-22", "2026-10-23"]));
    save(&lab, "recovery");
}

/// One logged HYROX race with labelled splits and the race extension, a
/// simulation two weeks earlier, and a training block of efforts.
pub fn hyrox() -> Lab {
    let mut lab = Lab::new();
    lab.write_one(
        &json!({ "id": "me", "record_type": "athlete_profile", "default_time_zone": "Europe/London", "sex": "male" }),
    );
    let runs = [255, 290, 300, 305, 315, 320, 325, 330];
    let stations = [
        ("ski_erg", 265),
        ("sled_push", 210),
        ("sled_pull", 280),
        ("burpee_broad_jumps", 300),
        ("row", 275),
        ("farmers_carry", 125),
        ("sandbag_lunges", 290),
        ("wall_balls", 420),
    ];
    let mut segments = Vec::new();
    for (index, (station, seconds)) in stations.iter().enumerate() {
        segments.push(json!({ "label": format!("run_{}", index + 1), "score": { "duration_s": runs[index] } }));
        segments.push(json!({ "label": station, "score": { "duration_s": seconds } }));
    }
    segments.push(json!({ "label": "roxzone", "score": { "duration_s": 380 } }));
    let total: i64 = runs.iter().sum::<i64>() + stations.iter().map(|(_, s)| s).sum::<i64>() + 380 + 15;
    lab.write_one(&json!({ "id": "race", "record_type": "benchmark_result", "benchmark": "hyrox-full",
        "benchmark_type": "hyrox", "at": "2026-10-18T11:30:00.000Z", "score": { "duration_s": total },
        "segments": segments,
        "extensions": { "org.athleticstandard/hyrox-race": { "season": "2026/27", "rulebook": "HYROX Rulebook Singles 26/27",
            "format": "singles", "division": "open", "category": "men", "event": "race", "age_group": "35-39", "penalties_s": 0 } } }));
    let first = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
    for week in 0..4 {
        for (offset, kind, minutes, rating) in [
            (0, "run intervals", 50, 7.0),
            (2, "station circuit", 60, 8.0),
            (4, "easy run", 40, 3.0),
            (5, "strength", 60, 6.0),
        ] {
            let day = first + Duration::days(week * 7 + offset);
            let id = format!("{kind}-{day}").replace(' ', "-");
            lab.write_one(&json!({ "id": id, "record_type": "workout_session", "source": "watch", "start": at(day, 7, 0),
                "end": at(day, 7, 0).replace("07:00", &format!("{:02}:{:02}", 7 + minutes / 60, minutes % 60)), "activity_type": kind }));
            lab.write_one(&json!({ "id": format!("effort-{id}"), "record_type": "athlete_report", "report_type": "session_effort",
                "at": at(day, 9, 0), "rating": rating, "rating_scale": "0-10", "duration_seconds": minutes * 60, "workout": id }));
        }
    }
    lab
}

#[test]
fn the_hyrox_scenario_has_the_facts_a_correct_answer_needs() {
    let mut lab = hyrox();
    let splits = lab.apply_shipped("hyrox-race-splits", "race");
    let result = &splits["result"];
    assert_eq!(number(&result["runs_seconds"]), 2440.0, "{splits:#}");
    assert_eq!(number(&result["stations_seconds"]), 2165.0);
    assert_eq!(number(&result["roxzone_seconds"]), 380.0);
    assert_eq!(number(&result["unaccounted_seconds"]), 15.0);
    assert_eq!(number(&result["run_summary"]["first_run_seconds"]), 255.0);
    assert!(number(&result["run_summary"]["later_runs_mean_seconds"]) > 300.0);
    assert_eq!(result["stations"][7]["label"], "wall_balls");
    assert_eq!(number(&result["stations"][7]["share_percent"]), 8.4);
    save(&lab, "hyrox");
}

/// Two Fran attempts that are not comparable, and a week in which a class
/// workout meant to last 7–10 minutes took 16 minutes.
pub fn crossfit() -> Lab {
    let mut lab = Lab::new();
    lab.write_one(&json!({ "id": "me", "record_type": "athlete_profile", "default_time_zone": "America/Denver", "sex": "female" }));
    lab.write_one(
        &json!({ "id": "fran-rx", "record_type": "benchmark_result", "benchmark": "fran", "benchmark_type": "named_wod",
        "at": "2026-10-03T15:00:00.000Z", "score": { "duration_s": 390 }, "scaling": "rx",
        "extensions": { "org.athleticstandard/crossfit-score": { "variant": "rx",
            "loads": [{ "movement": "thruster", "weight": 65, "weight_unit": "lb" }],
            "standards": "kipping pull-ups, chin over bar" } } }),
    );
    lab.write_one(&json!({ "id": "fran-scaled", "record_type": "benchmark_result", "benchmark": "fran", "benchmark_type": "named_wod",
        "at": "2026-10-31T15:00:00.000Z", "score": { "duration_s": 290 }, "scaling": "scaled",
        "extensions": { "org.athleticstandard/crossfit-score": { "variant": "scaled",
            "loads": [{ "movement": "thruster", "weight": 45, "weight_unit": "lb" }],
            "substitutions": [{ "prescribed": "pull-up", "performed": "ring row", "reason": "sore shoulder" }] } } }));
    for (date, text, minutes, rating) in
        [("2026-11-02", "16:12", 60, 9.0), ("2026-11-04", "15:40", 60, 9.0), ("2026-11-06", "17:05", 60, 10.0)]
    {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        lab.write_one(&json!({ "id": format!("class-{date}"), "record_type": "workout_session", "source": "watch",
            "start": at(day, 13, 0), "end": at(day, 14, 0), "activity_type": "crossfit", "name": "class",
            "description": "intended 7-10 minute couplet: 21-15-9 wall balls 14 lb and toes-to-bar", "text_result": text }));
        lab.write_one(
            &json!({ "id": format!("effort-{date}"), "record_type": "athlete_report", "report_type": "session_effort",
            "at": at(day, 14, 30), "rating": rating, "rating_scale": "0-10", "duration_seconds": minutes * 60,
            "workout": format!("class-{date}") }),
        );
    }
    lab
}

#[test]
fn the_crossfit_scenario_has_the_facts_a_correct_answer_needs() {
    let mut lab = crossfit();
    // The faster attempt is scaled with a substitution, so it is not a personal best.
    let scaled = lab.apply_shipped("training-load-effort", "effort-2026-11-06");
    assert_eq!(number(&scaled["result"]["load"]), 600.0, "{scaled:#}");
    save(&lab, "crossfit");
}
