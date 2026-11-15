//! Evidence, grading, and backtest replay, compared with 2.1.0 outputs.

#[path = "scenario_support/mod.rs"]
mod common;

use athleticstandard::json::{self, Value};
use athleticstandard::model::{EvidenceReference, RecordActor};
use athleticstandard::prediction::backtest::{run_backtest, BacktestOptions};
use athleticstandard::prediction::evidence::{build_evidence, resolve_evidence_reference, EvidenceRequest};
use athleticstandard::prediction::grade::{grade_prediction, plan_prediction_grade, GradeRequest};
use athleticstandard::runtime::model_client::{ModelError, ModelReply, PromptRequest};
use athleticstandard::store::Store;
use common::*;

fn tool() -> RecordActor {
    RecordActor::Tool { tool: "ath".into(), tool_version: athleticstandard::model::ATHLETIC_STANDARD_VERSION.into() }
}

fn grade_request(input: &Value) -> GradeRequest {
    GradeRequest {
        prediction_id: input["prediction_id"].as_str().unwrap().into(),
        result_id: input["result_id"].as_str().unwrap().into(),
        actor: tool(),
        calculation_version: opt_string(input, "calculation_version"),
        created_at: opt_string(input, "created_at"),
    }
}

fn prediction_call(store: &mut Store, call: &str, input: &Value) -> Value {
    let mut new_id = athleticstandard::ids::new_record_id;
    match call {
        "evidence" => outcome(build_evidence(
            store,
            &EvidenceRequest {
                benchmark: input["benchmark"].as_str().unwrap().into(),
                as_of: input["as_of"].as_str().unwrap().into(),
                limit: opt_u32(input, "limit"),
                window_days: opt_u32(input, "window_days"),
            },
        )),
        "evidence-reference" => {
            let reference: EvidenceReference = serde_json::from_value(input.clone()).unwrap();
            outcome(resolve_evidence_reference(store, &reference))
        }
        "grade-plan" => {
            let file = store.path().display().to_string();
            outcome(plan_prediction_grade(store, &grade_request(input), &mut new_id).map(|plan| plan.preview(&file)))
        }
        "grade-write" => {
            let plan = plan_prediction_grade(store, &grade_request(input), &mut new_id);
            outcome(plan.and_then(|plan| grade_prediction(store, &plan)).map(|written| {
                object(vec![
                    (
                        "link",
                        object(vec![
                            ("record_id", Value::String(written.link.record_id.clone())),
                            ("current_revision", Value::from(written.link.current_revision)),
                        ]),
                    ),
                    ("grade", written.grade.to_value()),
                    ("database_revision", Value::from(written.database_revision)),
                ])
            }))
        }
        "backtest" => {
            let replies = input["replies"].as_object().unwrap().clone();
            // The 2.1.0 request named its gateway. The native client is bound
            // to one gateway, so the recorded request names the one used here.
            let requests: std::cell::RefCell<Vec<Value>> = std::cell::RefCell::new(Vec::new());
            let client = |request: &PromptRequest<'_>| -> Result<ModelReply, ModelError> {
                requests.borrow_mut().push(object(vec![
                    ("gateway", Value::String("vercel".into())),
                    ("model", Value::String(request.model.into())),
                    ("system", Value::String(request.system.into())),
                    ("user", Value::String(request.user.into())),
                ]));
                match replies.get(request.model).and_then(Value::as_str) {
                    Some(reply) if !reply.starts_with("error:") => {
                        Ok(ModelReply { content: reply.to_string(), model: request.model.to_string() })
                    }
                    Some(reply) => Err(ModelError::Transport(reply[6..].to_string())),
                    None => Err(ModelError::Transport("no reply".into())),
                }
            };
            let options = BacktestOptions {
                models: input["models"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|model| model.as_str().unwrap().to_string())
                    .collect(),
                created_at: opt_string(input, "created_at"),
            };
            let report = run_backtest(store, &options, &client);
            let requests = requests.into_inner();
            outcome(
                report.map(|report| {
                    object(vec![("report", json::to_value(&report)), ("requests", Value::Array(requests))])
                }),
            )
        }
        "records-list" | "records-show" => {
            // Shared with the view tests; reproduced here to keep each test file standalone.
            use athleticstandard::model::RecordType;
            use athleticstandard::store::read::ListOptions;
            use athleticstandard::view::records::*;
            if call == "records-list" {
                let options = ListOptions {
                    record_type: opt_string(input, "record_type").map(|text| RecordType::parse(&text).unwrap()),
                    include_deleted: flag(input, "include_deleted"),
                    limit: opt_u32(input, "limit").unwrap_or(100),
                };
                outcome(list_records(store, &options).map(|list| {
                    object(vec![
                        ("list", json::to_value(&list)),
                        ("rendered", Value::String(render_record_list(&list.records))),
                    ])
                }))
            } else {
                let options = ShowOptions {
                    revision: input.get("revision").and_then(Value::as_u64),
                    history: flag(input, "history"),
                    links: flag(input, "links"),
                };
                outcome(show_record(store, input["record_id"].as_str().unwrap(), options).map(|show| {
                    object(vec![("show", json::to_value(&show)), ("rendered", Value::String(render_record(&show)))])
                }))
            }
        }
        other => panic!("unknown call {other}"),
    }
}

#[test]
fn evidence_and_backtest_match_the_reference() {
    Scenario::open("prediction/evidence.json")
        .check(|store, call, input, _| (prediction_call(store, call, input), Compare::Exact));
}

#[test]
fn grading_matches_the_reference() {
    Scenario::open("prediction/grade.json")
        .check(|store, call, input, _| (prediction_call(store, call, input), Compare::Exact));
}
