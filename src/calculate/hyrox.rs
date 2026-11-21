//! `hyrox_splits`: one HYROX result divided into runs, stations, Roxzone, and
//! penalties without counting time twice (D245).
//!
//! The result is a `benchmark_result` whose `segments` carry the labels below
//! and a `score.duration_s` each, and whose `score.duration_s` is the finish
//! time. The 2026/27 rulebooks do not define where Roxzone timing starts or
//! ends, and timing providers split it differently, so this method accepts
//! either one `roxzone` total or numbered `roxzone_N` transitions, never both.
//! Season, format, division, and penalties come from the
//! `org.athleticstandard/hyrox-race` extension.

use serde::Serialize;

use super::record::{Context, FieldRead, OneRecordResult, StoredRecord};
use crate::json::Value;

/// The extension that names the season, format, division, and penalties.
pub const HYROX_EXTENSION: &str = "org.athleticstandard/hyrox-race";

/// The eight stations in race order.
pub const STATIONS: [&str; 8] =
    ["ski_erg", "sled_push", "sled_pull", "burpee_broad_jumps", "row", "farmers_carry", "sandbag_lunges", "wall_balls"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Split {
    pub label: String,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StationSplit {
    pub label: String,
    pub seconds: f64,
    pub share_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSummary {
    pub count: usize,
    pub mean_seconds: f64,
    pub fastest: Split,
    pub slowest: Split,
    pub spread_seconds: f64,
    /// Run 1, which follows no station.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_run_seconds: Option<f64>,
    /// The mean of the runs that follow a station.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub later_runs_mean_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SharePercent {
    pub runs: f64,
    pub stations: f64,
    pub roxzone: f64,
    pub penalties: f64,
    pub unaccounted: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HyroxSplitsResult {
    pub total_seconds: f64,
    pub runs_seconds: f64,
    pub stations_seconds: f64,
    pub roxzone_seconds: f64,
    pub penalties_seconds: f64,
    /// Finish time not covered by any part, such as Roxzone time that was not
    /// recorded.
    pub unaccounted_seconds: f64,
    pub share_percent: SharePercent,
    pub runs: Vec<Split>,
    pub run_summary: Option<RunSummary>,
    pub stations: Vec<StationSplit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub division: Option<String>,
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

enum Part {
    Run(usize),
    Station(usize),
    RoxzoneTotal,
    Roxzone,
    Penalty,
}

fn part_of(label: &str) -> Option<Part> {
    if let Some(number) = label.strip_prefix("run_").and_then(|rest| rest.parse::<usize>().ok()) {
        return (1..=8).contains(&number).then_some(Part::Run(number));
    }
    if let Some(number) = label.strip_prefix("roxzone_").and_then(|rest| rest.parse::<usize>().ok()) {
        return (1..=16).contains(&number).then_some(Part::Roxzone);
    }
    match label {
        "roxzone" => Some(Part::RoxzoneTotal),
        "penalty" | "penalties" => Some(Part::Penalty),
        other => STATIONS.iter().position(|station| *station == other).map(Part::Station),
    }
}

pub(super) fn hyrox_splits(context: &mut Context<'_>, record: &StoredRecord) -> Option<OneRecordResult> {
    if record.record_type != "benchmark_result" {
        context.missing.push(format!("a benchmark_result; this record is a {}", record.record_type));
        return None;
    }
    let total = context.data.get("score").and_then(|score| score.get("duration_s")).and_then(Value::as_f64);
    context.fields_read.push(FieldRead { field: "score.duration_s".into(), present: total.is_some(), value: None });
    let segments = context.lookup("segments").and_then(Value::as_array).cloned();
    let extension = record.extensions.as_ref().and_then(|extensions| extensions.get(HYROX_EXTENSION)).cloned();
    context.fields_read.push(FieldRead {
        field: format!("extensions.{HYROX_EXTENSION}"),
        present: extension.is_some(),
        value: None,
    });
    let Some(total) = total else {
        context.missing.push("score.duration_s, the finish time".into());
        return None;
    };
    let Some(segments) = segments.filter(|segments| !segments.is_empty()) else {
        context.missing.push("segments labelled run_1 to run_8, the stations, and Roxzone".into());
        return None;
    };
    let mut runs: Vec<(usize, Split)> = Vec::new();
    let mut stations: Vec<(usize, f64)> = Vec::new();
    let (mut roxzone_total, mut roxzone_parts, mut penalty_segments) = (None, Vec::new(), 0.0);
    let mut labels = std::collections::BTreeSet::new();
    for segment in &segments {
        let label = segment.get("label").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
        let Some(part) = part_of(&label) else {
            context.missing.push(format!(
                "segment '{label}' is not a HYROX split label (run_1 to run_8, {}, roxzone or roxzone_1 to roxzone_16, penalty)",
                STATIONS.join(", ")
            ));
            return None;
        };
        if !labels.insert(label.clone()) {
            context.missing.push(format!("segment '{label}' appears twice, so its time would be counted twice"));
            return None;
        }
        let Some(seconds) = segment.get("score").and_then(|score| score.get("duration_s")).and_then(Value::as_f64)
        else {
            context.missing.push(format!("score.duration_s on segment '{label}'"));
            return None;
        };
        match part {
            Part::Run(number) => runs.push((number, Split { label, seconds })),
            Part::Station(index) => stations.push((index, seconds)),
            Part::RoxzoneTotal => roxzone_total = Some(seconds),
            Part::Roxzone => roxzone_parts.push(seconds),
            Part::Penalty => penalty_segments += seconds,
        }
    }
    if roxzone_total.is_some() && !roxzone_parts.is_empty() {
        context
            .missing
            .push("either one roxzone total or numbered roxzone_N transitions; both would count Roxzone twice".into());
        return None;
    }
    let extension_penalty =
        extension.as_ref().and_then(|value| value.get("penalties_s")).and_then(Value::as_f64).unwrap_or(0.0);
    if extension_penalty > 0.0 && penalty_segments > 0.0 {
        context.missing.push("penalties as a segment or as penalties_s in the extension, not both".into());
        return None;
    }
    runs.sort_by_key(|(number, _)| *number);
    stations.sort_by_key(|(index, _)| *index);
    let runs_seconds: f64 = runs.iter().map(|(_, split)| split.seconds).sum();
    let stations_seconds: f64 = stations.iter().map(|(_, seconds)| seconds).sum();
    let roxzone_seconds = roxzone_total.unwrap_or_else(|| roxzone_parts.iter().sum());
    let penalties_seconds = penalty_segments + extension_penalty;
    let parts = runs.len() + stations.len() + roxzone_parts.len() + usize::from(roxzone_total.is_some()) + 1;
    // Each split may be rounded to the second.
    let tolerance = 0.5 * parts as f64 + 0.5;
    let sum = runs_seconds + stations_seconds + roxzone_seconds + penalties_seconds;
    if sum > total + tolerance {
        context.missing.push(format!(
            "parts that add up to no more than the finish time; they add up to {sum:.1} s against {total:.1} s, so some time is counted twice"
        ));
        return None;
    }
    let unaccounted = (total - sum).max(0.0);
    if runs.len() < 8 {
        context.exclusions.push(format!("{} of 8 runs recorded", runs.len()));
    }
    if stations.len() < 8 {
        context.exclusions.push(format!("{} of 8 stations recorded", stations.len()));
    }
    if roxzone_total.is_none() && roxzone_parts.is_empty() {
        context.exclusions.push("no Roxzone time recorded; it is part of the unaccounted time".into());
    }
    let share = |seconds: f64| round(seconds / total * 100.0);
    let run_summary = (!runs.is_empty()).then(|| {
        let splits: Vec<&Split> = runs.iter().map(|(_, split)| split).collect();
        let fastest = splits.iter().min_by(|a, b| a.seconds.total_cmp(&b.seconds)).map(|split| (*split).clone());
        let slowest = splits.iter().max_by(|a, b| a.seconds.total_cmp(&b.seconds)).map(|split| (*split).clone());
        let (fastest, slowest) = (fastest.unwrap(), slowest.unwrap());
        let first = runs.iter().find(|(number, _)| *number == 1).map(|(_, split)| split.seconds);
        let later: Vec<f64> = runs.iter().filter(|(number, _)| *number > 1).map(|(_, split)| split.seconds).collect();
        RunSummary {
            count: runs.len(),
            mean_seconds: round(runs_seconds / runs.len() as f64),
            spread_seconds: round(slowest.seconds - fastest.seconds),
            fastest,
            slowest,
            first_run_seconds: first,
            later_runs_mean_seconds: (!later.is_empty()).then(|| round(later.iter().sum::<f64>() / later.len() as f64)),
        }
    });
    let text =
        |key: &str| extension.as_ref().and_then(|value| value.get(key)).and_then(Value::as_str).map(str::to_string);
    Some(OneRecordResult::HyroxSplits(HyroxSplitsResult {
        total_seconds: total,
        runs_seconds: round(runs_seconds),
        stations_seconds: round(stations_seconds),
        roxzone_seconds: round(roxzone_seconds),
        penalties_seconds: round(penalties_seconds),
        unaccounted_seconds: round(unaccounted),
        share_percent: SharePercent {
            runs: share(runs_seconds),
            stations: share(stations_seconds),
            roxzone: share(roxzone_seconds),
            penalties: share(penalties_seconds),
            unaccounted: share(unaccounted),
        },
        runs: runs.into_iter().map(|(_, split)| split).collect(),
        run_summary,
        stations: stations
            .into_iter()
            .map(|(index, seconds)| StationSplit {
                label: STATIONS[index].to_string(),
                seconds,
                share_percent: share(seconds),
            })
            .collect(),
        season: text("season"),
        format: text("format"),
        division: text("division"),
    }))
}
