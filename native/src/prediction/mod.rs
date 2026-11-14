//! The reference agent's record rules: offline evidence, prediction writes,
//! result linking, deterministic grading, and historical replay.
//!
//! Network calls are not made here. Prediction and backtest receive a
//! `ModelClient`, which the runtime module implements.

pub mod backtest;
pub mod benchmarks;
pub mod client;
pub mod evidence;
pub mod grade;
pub mod predict;

pub use backtest::{run_backtest, write_backtest_report, BacktestOptions, BacktestReplay, BacktestReport};
pub use client::{CompleteRequest, CompleteResponse, Gateway, ModelClient};
pub use evidence::{build_evidence, resolve_evidence_reference, EvidenceRequest, PredictionEvidence};
pub use grade::{grade_prediction, plan_prediction_grade, GradePreview, GradeRequest, PredictionGradePlan};
pub use predict::{request_prediction, write_prediction, PredictionPreview};

use crate::error::Result;
use crate::store::Store;
use crate::view::calendar::{resolve_as_of_time, AsOfTime};

/// `ath predict --json`: evidence for an agent. `as_of` is a calendar day, an
/// exact UTC instant, or now. A calendar day uses `time_zone`, else the
/// athlete profile's timezone, and the evidence names the timezone used.
pub fn prediction_evidence(
    store: &Store,
    benchmark: &str,
    as_of: Option<&str>,
    time_zone: Option<&str>,
) -> Result<PredictionEvidence> {
    let AsOfTime { as_of, time_zone } =
        resolve_as_of_time(store.connection(), as_of, time_zone, crate::model::time::now_utc)?;
    let mut evidence = build_evidence(store.connection(), &EvidenceRequest::new(benchmark, as_of))?;
    evidence.calendar_time_zone = time_zone;
    Ok(evidence)
}
