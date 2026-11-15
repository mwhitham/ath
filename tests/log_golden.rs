//! `ath log` planning, compared with 2.1.0 outputs.
//!
//! The 2.1.0 preview printed each planned record's data in the order the
//! planner built it, which differs from the published field order for a few
//! fields such as `field_basis`. The native preview uses the published order,
//! the same order the stored record has. These cases therefore compare with
//! sorted object keys. Values, record types, and interpretations must match.

#[path = "scenario_support/mod.rs"]
mod common;

use athleticstandard::json::Value;
use athleticstandard::log::{apply_log_plan, plan_log_entry, LogRequest};
use athleticstandard::model::RecordActor;
use athleticstandard::store::Store;
use common::*;

fn request(input: &Value) -> LogRequest {
    LogRequest {
        text: input["text"].as_str().unwrap().into(),
        actor: serde_json::from_value::<RecordActor>(input["actor"].clone()).unwrap(),
        created_at: opt_string(input, "created_at"),
        benchmark: opt_string(input, "benchmark"),
    }
}

fn log_call(store: &mut Store, call: &str, input: &Value) -> Value {
    let mut new_id = athleticstandard::ids::new_record_id;
    let plan = plan_log_entry(store, &request(input), &mut new_id);
    match call {
        "log-plan" => outcome(plan.map(|plan| {
            let preview = athleticstandard::json::to_value(&plan.preview("unused"));
            object(vec![("interpretation", preview["interpretation"].clone()), ("records", preview["records"].clone())])
        })),
        "log-write" => outcome(plan.and_then(|plan| {
            let written = apply_log_plan(store, &plan)?;
            Ok(object(vec![
                ("interpretation", Value::String(plan.interpretation.clone())),
                (
                    "records",
                    Value::Array(
                        written
                            .records
                            .iter()
                            .map(|record| Value::String(record.record_type().as_str().into()))
                            .collect(),
                    ),
                ),
                ("database_revision", Value::from(written.database_revision)),
            ]))
        })),
        other => panic!("unknown call {other}"),
    }
}

#[test]
fn log_planning_matches_the_reference() {
    Scenario::open("log/log.json").check(|store, call, input, _| (log_call(store, call, input), Compare::SortedKeys));
}
