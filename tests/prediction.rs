//! Prediction evidence, prediction writes, grading, and backtest replay.
//!
//! Ports `tests/v1-predict-backtest.test.ts` and adds the replay cases the
//! 2.1.1 plan requires: corrected outcomes, late imports, changed
//! definitions, deleted evidence, and provider periods.

#[path = "scenario_support/mod.rs"]
mod common;

use std::collections::HashMap;

use athleticstandard::json::{self, Value};
use athleticstandard::model::{RecordActor, RecordData};
use athleticstandard::prediction::backtest::{report_path, run_backtest, write_backtest_report, BacktestOptions};
use athleticstandard::prediction::evidence::{build_evidence, EvidenceRequest};
use athleticstandard::prediction::grade::{grade_prediction, plan_prediction_grade, GradeRequest, LinkAction};
use athleticstandard::prediction::predict::{request_prediction, write_prediction};
use athleticstandard::prediction::prediction_evidence;
use athleticstandard::runtime::model_client::{ModelClient, ModelError, ModelReply, PromptRequest};
use athleticstandard::store::Store;
use athleticstandard::Error;
use common::builders::*;
use common::*;

/// One request a fake model received.
#[derive(Debug, Clone)]
struct Asked {
    model: String,
    user: String,
}

/// A model that answers from a table and remembers what it was asked.
#[derive(Default)]
struct FakeModel {
    replies: HashMap<String, std::result::Result<String, String>>,
    requests: std::cell::RefCell<Vec<Asked>>,
}

impl FakeModel {
    fn answering(model: &str, reply: &str) -> Self {
        let mut fake = FakeModel::default();
        fake.replies.insert(model.into(), Ok(reply.into()));
        fake
    }

    fn evidence(&self, index: usize) -> Value {
        json::parse(&self.requests.borrow()[index].user).unwrap()
    }
}

impl ModelClient for FakeModel {
    fn complete(&self, request: &PromptRequest<'_>) -> std::result::Result<ModelReply, ModelError> {
        self.requests.borrow_mut().push(Asked { model: request.model.to_string(), user: request.user.to_string() });
        match self.replies.get(request.model) {
            Some(Ok(content)) => Ok(ModelReply { content: content.clone(), model: request.model.to_string() }),
            Some(Err(message)) => Err(ModelError::Transport(message.clone())),
            None => Err(ModelError::Transport("no reply".into())),
        }
    }
}

fn ids(records: &Value) -> Vec<String> {
    records.as_array().unwrap().iter().map(|record| record["record_id"].as_str().unwrap().to_string()).collect()
}

fn revisions(records: &Value) -> Vec<(String, u64)> {
    records
        .as_array()
        .unwrap()
        .iter()
        .map(|record| (record["record_id"].as_str().unwrap().to_string(), record["current_revision"].as_u64().unwrap()))
        .collect()
}

fn options(models: &[&str]) -> BacktestOptions {
    BacktestOptions {
        models: models.iter().map(|model| model.to_string()).collect(),
        created_at: Some("2026-10-13T12:00:00.000Z".into()),
    }
}

const REPLY_290: &str = r#"{"predicted_score":{"duration_s":290},"confidence":"moderate","reasoning":"x"}"#;

#[test]
fn evidence_is_offline_and_names_its_benchmark() {
    let (_directory, store) = replay_demo();
    let evidence = prediction_evidence(&store, "fran", Some("2026-08-11T12:00:00.000Z"), None).unwrap();
    let value = json::to_value(&evidence);
    assert_eq!(value["purpose"], "prediction_evidence");
    assert_eq!(value["is_prediction"], false);
    assert_eq!(value["benchmark"]["record_type"], "benchmark_definition");
    assert_eq!(value["benchmark"]["data"]["benchmark_key"], "fran");
    for list in ["prior_results", "device_measurements", "athlete_reports", "gaps"] {
        assert!(value[list].is_array(), "{list}");
    }
    assert!(value.get("calendar_time_zone").is_none());
    let calendar = prediction_evidence(&store, "fran", Some("2026-08-11"), Some("America/Chicago")).unwrap();
    assert_eq!(calendar.as_of, "2026-08-12T04:59:59.999Z");
    assert_eq!(calendar.calendar_time_zone.as_deref(), Some("America/Chicago"));
    assert!(!calendar.model_text().contains("calendar_time_zone"));
}

#[test]
fn a_backtest_replays_a_later_result_and_writes_only_its_report() {
    let (directory, mut store) = replay_demo();
    replay(
        &mut store,
        &[write(vec![result(
            "01890f3e-e7d3-7cc0-88c8-0000000000bb",
            "01890f3e-e7d3-7cc0-88c8-000000000004",
            "01890f3e-e7d3-7cc0-88c8-000000000006",
            "2026-08-11T17:55:00.000Z",
            295.0,
        )])],
    );
    let before = store.database_revision().unwrap();
    let model = FakeModel::answering("test-model", REPLY_290);
    let report = run_backtest(&store, &options(&["test-model"]), &model).unwrap();
    assert_eq!((report.replayable_result_count, report.skipped_first_result_count), (1, 1));
    let replay = &report.replays[0];
    assert_eq!((replay.predicted_value, replay.actual_value), (Some(290.0), 295.0));
    assert_eq!(replay.absolute_error_percentage, Some(1.7));
    assert_eq!(report.summary[0].completed, 1);
    assert_eq!(report.summary[0].failed, 0);
    assert_eq!(report.summary[0].median_absolute_error_percentage, Some(1.7));
    assert_eq!(store.database_revision().unwrap(), before);

    let path = write_backtest_report(store.path(), &report).unwrap();
    assert_eq!(path, directory.join("backtest-2026-10-13T120000000Z.json"));
    assert_eq!(path, report_path(store.path(), &report.created_at));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with("}\n"));
    assert_eq!(json::to_string(&json::parse(&text).unwrap()), json::encode(&report));
    assert!(write_backtest_report(store.path(), &report).is_err(), "an existing report is never replaced");
    assert_eq!(store.database_revision().unwrap(), before);
}

#[test]
fn model_failures_are_recorded_and_the_other_models_continue() {
    let (_directory, mut store) = replay_demo();
    replay(
        &mut store,
        &[write(vec![result(
            &id(0xbb),
            "01890f3e-e7d3-7cc0-88c8-000000000004",
            "01890f3e-e7d3-7cc0-88c8-000000000006",
            "2026-08-11T17:55:00.000Z",
            295.0,
        )])],
    );
    let mut model = FakeModel::answering("good", REPLY_290);
    model.replies.insert("down".into(), Err("gateway timeout".into()));
    model.replies.insert("chatty".into(), Ok("I think about 4:50".into()));
    model.replies.insert("wrong-unit".into(), Ok(r#"{"predicted_score":{"reps":12}}"#.into()));
    let report = run_backtest(&store, &options(&["good", "down", "chatty", "wrong-unit"]), &model).unwrap();
    let errors: Vec<Option<&str>> = report.replays.iter().map(|replay| replay.error.as_deref()).collect();
    assert_eq!(errors[0], None);
    assert_eq!(errors[1], Some("gateway timeout"));
    assert!(errors[2].is_some());
    assert_eq!(errors[3], Some("score does not contain time"));
    let failed: Vec<usize> = report.summary.iter().map(|summary| summary.failed).collect();
    assert_eq!(failed, vec![0, 1, 1, 1]);
    assert_eq!(report.summary[1].median_absolute_error_percentage, None);
}

/// Benchmark `fran` with three results, each logged when it happened.
fn replay_database() -> (std::path::PathBuf, Store) {
    let (directory, mut store) = new_store("2026-01-01T00:00:00.000Z");
    replay(
        &mut store,
        &[
            write(vec![source(&id(0x101), "Device A", "wearable"), source(&id(0x103), "ath", "manual")]),
            write(vec![definition(&id(0x901), "fran", "21-15-9 thrusters and pull-ups")]),
            write(vec![result(&id(0xb01), &id(0x103), &id(0x901), "2026-06-01T10:00:00.000Z", 300.0)]),
        ],
    );
    (directory, store)
}

#[test]
fn a_corrected_outcome_never_shows_itself_to_the_model() {
    // 2.1.0 replayed a corrected result from just before its correction became
    // known, so the model saw the result's own first revision as a prior result.
    let (_directory, mut store) = replay_database();
    let first = result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 300.0);
    let mut corrected = first["data"].clone();
    corrected["score"]["duration_s"] = Value::from(295);
    replay(&mut store, &[write(vec![first]), correct(&id(0xb02), 1, "2026-08-01T00:00:00.000Z", &corrected)]);
    let model = FakeModel::answering("m", REPLY_290);
    let report = run_backtest(&store, &options(&["m"]), &model).unwrap();
    assert_eq!(report.replays.len(), 1);
    assert_eq!(report.replays[0].as_of, "2026-07-01T09:59:59.999Z");
    assert_eq!(report.replays[0].actual_value, 295.0);
    let evidence = model.evidence(0);
    assert_eq!(evidence["as_of"], "2026-07-01T09:59:59.999Z");
    assert_eq!(ids(&evidence["prior_results"]), vec![id(0xb01)]);
}

#[test]
fn a_late_import_sees_only_what_was_known_before_the_result() {
    // A result logged late (recorded in July, logged in September) must not see
    // the August result, and the August replay must not see the July result.
    let (_directory, mut store) = replay_database();
    replay(
        &mut store,
        &[
            write(vec![sleep(&id(0xc01), &id(0x101), "2026-06-20T04:00:00.000Z", "2026-06-20T11:00:00.000Z")]),
            write(vec![result(&id(0xb03), &id(0x103), &id(0x901), "2026-08-01T10:00:00.000Z", 289.0)]),
            write(vec![known(
                result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0),
                "2026-09-01T00:00:00.000Z",
            )]),
            write(vec![known(
                sleep(&id(0xc02), &id(0x101), "2026-06-25T04:00:00.000Z", "2026-06-25T11:00:00.000Z"),
                "2026-09-02T00:00:00.000Z",
            )]),
        ],
    );
    let model = FakeModel::answering("m", REPLY_290);
    let report = run_backtest(&store, &options(&["m"]), &model).unwrap();
    let by_result: HashMap<&str, (&str, usize)> = report
        .replays
        .iter()
        .enumerate()
        .map(|(index, replay)| (replay.benchmark_result_id.as_str(), (replay.as_of.as_str(), index)))
        .collect();
    let (july_as_of, july) = by_result[id(0xb02).as_str()];
    let (august_as_of, august) = by_result[id(0xb03).as_str()];
    assert_eq!(july_as_of, "2026-07-01T09:59:59.999Z");
    assert_eq!(august_as_of, "2026-08-01T09:59:59.999Z");
    // The July replay sees only what happened and was known before July 1.
    assert_eq!(ids(&model.evidence(july)["prior_results"]), vec![id(0xb01)]);
    // The August replay does not see the July result: it was learned in September.
    assert_eq!(ids(&model.evidence(august)["prior_results"]), vec![id(0xb01)]);
    for index in [july, august] {
        assert_eq!(ids(&model.evidence(index)["sleep_sessions"]), vec![id(0xc01)], "late sleep import leaked");
    }
}

#[test]
fn a_changed_definition_shows_the_revision_known_at_the_cut_off() {
    let (_directory, mut store) = replay_database();
    let mut changed = definition(&id(0x901), "fran", "21-15-9 thrusters 43 kg and pull-ups")["data"].clone();
    changed["benchmark_key"] = Value::String("fran-rx".into());
    replay(
        &mut store,
        &[
            write(vec![result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0)]),
            correct(&id(0x901), 1, "2026-07-15T00:00:00.000Z", &changed),
        ],
    );
    let model = FakeModel::answering("m", REPLY_290);
    let report = run_backtest(&store, &options(&["m"]), &model).unwrap();
    let evidence = model.evidence(0);
    assert_eq!(evidence["benchmark"]["current_revision"], 1);
    assert_eq!(evidence["benchmark"]["data"]["definition"], "21-15-9 thrusters and pull-ups");
    // The report names the benchmark as it is now.
    assert_eq!(report.replays[0].benchmark_key, "fran-rx");
}

#[test]
fn deleted_evidence_disappears_only_once_the_deletion_was_known() {
    let (_directory, mut store) = replay_database();
    replay(
        &mut store,
        &[
            write(vec![point(&id(0xa01), &id(0x101), "hrv_rmssd", "2026-06-10T12:00:00.000Z", 61.0, "ms")]),
            write(vec![point(&id(0xa02), &id(0x101), "hrv_rmssd", "2026-06-11T12:00:00.000Z", 59.0, "ms")]),
            delete(&id(0xa01), 1, "2026-06-20T00:00:00.000Z"),
            write(vec![result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0)]),
            delete(&id(0xa02), 1, "2026-07-05T00:00:00.000Z"),
        ],
    );
    let model = FakeModel::answering("m", REPLY_290);
    run_backtest(&store, &options(&["m"]), &model).unwrap();
    // Deleted before the cut-off: hidden. Deleted after it: what was known then.
    assert_eq!(ids(&model.evidence(0)["device_measurements"]), vec![id(0xa02)]);
    let now = build_evidence(&store, &EvidenceRequest::new("fran", "2026-07-06T00:00:00.000Z")).unwrap();
    assert!(now.device_measurements.is_empty());
}

#[test]
fn corrections_after_the_cut_off_never_leak_into_evidence() {
    let (_directory, mut store) = replay_database();
    let original = point(&id(0xa01), &id(0x101), "hrv_rmssd", "2026-06-10T12:00:00.000Z", 61.0, "ms");
    let mut corrected = original["data"].clone();
    corrected["value"] = value(66.0, "ms");
    replay(&mut store, &[write(vec![original]), correct(&id(0xa01), 1, "2026-07-10T00:00:00.000Z", &corrected)]);
    let before = build_evidence(&store, &EvidenceRequest::new("fran", "2026-07-01T00:00:00.000Z")).unwrap();
    let after = build_evidence(&store, &EvidenceRequest::new("fran", "2026-07-11T00:00:00.000Z")).unwrap();
    let before = json::to_value(&before);
    let after = json::to_value(&after);
    assert_eq!(revisions(&before["device_measurements"]), vec![(id(0xa01), 1)]);
    assert_eq!(json::to_string(&before["device_measurements"][0]["data"]["value"]["standard_value"]), "61");
    assert_eq!(revisions(&after["device_measurements"]), vec![(id(0xa01), 2)]);
}

#[test]
fn provider_period_values_never_become_dated_evidence() {
    let (_directory, mut store) = replay_database();
    replay(
        &mut store,
        &[
            write(vec![period_point(&id(0xa01), &id(0x101), "2026-06-20", 57.0, "2026-06-21T12:00:00.000Z")]),
            write(vec![point(&id(0xa02), &id(0x101), "hrv_rmssd", "2026-06-22T12:00:00.000Z", 59.0, "ms")]),
            write(vec![result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0)]),
        ],
    );
    let model = FakeModel::answering("m", REPLY_290);
    run_backtest(&store, &options(&["m"]), &model).unwrap();
    assert_eq!(ids(&model.evidence(0)["device_measurements"]), vec![id(0xa02)]);
}

#[test]
fn first_results_are_anchors_and_deleted_results_are_not_replayed() {
    let (_directory, mut store) = replay_database();
    replay(
        &mut store,
        &[
            write(vec![definition(&id(0x902), "grace", "30 clean and jerks")]),
            write(vec![result(&id(0xb04), &id(0x103), &id(0x902), "2026-06-15T10:00:00.000Z", 200.0)]),
            write(vec![result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0)]),
            write(vec![result(&id(0xb03), &id(0x103), &id(0x901), "2026-08-01T10:00:00.000Z", 289.0)]),
            delete(&id(0xb03), 1, "2026-08-02T00:00:00.000Z"),
        ],
    );
    let model = FakeModel::answering("m", REPLY_290);
    let report = run_backtest(&store, &options(&["m"]), &model).unwrap();
    assert_eq!((report.replayable_result_count, report.skipped_first_result_count), (1, 2));
    assert_eq!(report.replays[0].benchmark_result_id, id(0xb02));
    assert!(run_backtest(&store, &options(&[]), &model).is_err());
}

#[test]
fn a_model_reply_cannot_claim_authorship_or_move_the_benchmark() {
    let (_directory, store) = replay_database();
    let evidence = build_evidence(&store, &EvidenceRequest::new("fran", "2026-06-02T00:00:00.000Z")).unwrap();
    let claim = r#"{"authorship":{"author":{"author_type":"athlete"},"authored_at":"2026-06-02T00:00:00.000Z","evidence":[],"content_version":1,"status":"active"},"predicted_score":{"duration_s":280},"confidence":"high","reasoning":"x"}"#;
    let model = FakeModel::answering("m", claim);
    let error = request_prediction(&evidence, &model, "m", "2026-06-02T00:00:00.000Z").unwrap_err();
    assert_eq!(
        error.to_string(),
        "model prediction response cannot set authorship; ath sets the authorship and benchmark"
    );
    let moved = format!(r#"{{"benchmark_definition_id":"{}","predicted_score":{{"duration_s":280}}}}"#, id(0x902));
    let model = FakeModel::answering("m", &moved);
    let error = request_prediction(&evidence, &model, "m", "2026-06-02T00:00:00.000Z").unwrap_err();
    assert!(error.to_string().contains("cannot set benchmark_definition_id"));
}

#[test]
fn a_failed_model_call_or_invalid_reply_writes_nothing() {
    let (_directory, store) = replay_database();
    let before = store.database_revision().unwrap();
    let evidence = build_evidence(&store, &EvidenceRequest::new("fran", "2026-06-02T00:00:00.000Z")).unwrap();
    let mut model = FakeModel::default();
    model.replies.insert("m".into(), Err("connection reset".into()));
    let error = request_prediction(&evidence, &model, "m", "2026-06-02T00:00:00.000Z").unwrap_err();
    assert_eq!(error.to_string(), "connection reset");
    assert_eq!(model.requests.borrow()[0].model, "m");
    let model = FakeModel::answering("m", r#"{"predicted_score":{"duration_s":280}}"#);
    let error = request_prediction(&evidence, &model, "m", "2026-06-02T00:00:00.000Z").unwrap_err();
    assert!(matches!(error, Error::Invalid(_)), "{error}");
    assert_eq!(store.database_revision().unwrap(), before);
}

#[test]
fn a_checked_prediction_is_written_as_one_agent_record() {
    let (_directory, mut store) = replay_database();
    let evidence = build_evidence(&store, &EvidenceRequest::new("fran", "2026-06-02T00:00:00.000Z")).unwrap();
    let reply = "```json\n{\"predicted_score\":{\"duration_s\":280},\"predicted_range\":{\"low\":{\"duration_s\":270},\"high\":{\"duration_s\":290}},\"confidence\":\"moderate\",\"reasoning\":\"Anchored on the June result.\"}\n```";
    let model = FakeModel::answering("test-model", reply);
    let preview = request_prediction(&evidence, &model, "test-model", "2026-06-02T00:00:00.000Z").unwrap();
    assert_eq!(model.requests.borrow()[0].user, evidence.model_text());
    assert_eq!(preview.benchmark, "fran");
    assert!(!preview.write_confirmed);
    assert_eq!(preview.data.authorship.evidence, evidence.references());
    let author = json::to_value(&preview.data.authorship.author);
    assert_eq!(author["author_type"], "agent");
    assert_eq!(author["agent"], "ath-terminal");
    assert_eq!(author["model"], "test-model");
    let before = store.database_revision().unwrap();
    let written = write_prediction(&mut store, &preview).unwrap();
    assert_eq!(written.database_revision, before + 1);
    assert_eq!(written.record.created_at, "2026-06-02T00:00:00.000Z");
    let RecordData::Prediction(saved) = &written.record.data else { panic!("prediction") };
    assert_eq!(saved, &preview.data);
    let revision = store.revisions(&written.record.record_id).unwrap().remove(0);
    assert_eq!(json::to_value(&revision.changed_by)["agent"], "ath-terminal");
}

fn tool_actor() -> RecordActor {
    athleticstandard::prediction::grade::tool_actor()
}

#[test]
fn grading_creates_a_link_then_reuses_it_and_refuses_a_stale_plan() {
    let (_directory, mut store) = replay_database();
    replay(
        &mut store,
        &[
            write(vec![prediction(&id(0xf01), &id(0x901), "2026-06-30T12:00:00.000Z", 290.0)]),
            write(vec![result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 295.0)]),
        ],
    );
    let request = GradeRequest {
        prediction_id: id(0xf01),
        result_id: id(0xb02),
        actor: tool_actor(),
        calculation_version: None,
        created_at: Some("2026-07-02T00:00:00.000Z".into()),
    };
    let mut new_id = athleticstandard::ids::new_record_id;
    let plan = plan_prediction_grade(&store, &request, &mut new_id).unwrap();
    let LinkAction::Create { record_id: link_id, .. } = plan.preview("file").prediction_result_link else {
        panic!("a first grade creates its link")
    };
    let written = grade_prediction(&mut store, &plan).unwrap();
    assert_eq!(written.link.record_id, link_id);
    assert_eq!(written.summary_line(), "hit — signed error -5, absolute error 1.7%");

    let again = plan_prediction_grade(&store, &request, &mut new_id).unwrap();
    assert!(matches!(again.preview("file").prediction_result_link, LinkAction::Reuse { revision: 1, .. }));

    let mut corrected = result(&id(0xb02), &id(0x103), &id(0x901), "2026-07-01T10:00:00.000Z", 301.0)["data"].clone();
    corrected["field_basis"] = serde_json::json!({ "value": "entered_by_hand" });
    replay(&mut store, &[correct(&id(0xb02), 1, "2026-07-03T00:00:00.000Z", &corrected)]);
    let before = store.database_revision().unwrap();
    let error = grade_prediction(&mut store, &again).unwrap_err();
    assert!(matches!(error, Error::Stale(_)), "{error}");
    assert_eq!(store.database_revision().unwrap(), before);
}

#[test]
fn an_erased_evidence_reference_resolves_to_its_value_free_marker() {
    use athleticstandard::model::EvidenceReference;
    use athleticstandard::prediction::evidence::resolve_evidence_reference;
    let (_directory, store) = replay_database();
    write_directly(store.path(), |tx| {
        tx.execute(
            "INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) VALUES (?1, ?2, 'athlete_request')",
            [id(0xa09), "2026-07-01T00:00:00.000Z".to_string()],
        )?;
        Ok(())
    });
    let erased = resolve_evidence_reference(&store, &EvidenceReference { record_id: id(0xa09), revision: 1 }).unwrap();
    assert_eq!(
        json::encode(&erased),
        format!(
            r#"{{"status":"erased","reference":{{"record_id":"{0}","revision":1}},"erasure_marker":{{"record_id":"{0}","erased_at":"2026-07-01T00:00:00.000Z","reason_code":"athlete_request"}}}}"#,
            id(0xa09)
        )
    );
    let available =
        resolve_evidence_reference(&store, &EvidenceReference { record_id: id(0xb01), revision: 1 }).unwrap();
    assert_eq!(json::to_value(&available)["status"], "available");
}

#[test]
fn an_as_of_query_takes_one_type_or_a_list() {
    use athleticstandard::model::RecordType;
    use athleticstandard::store::query::AsOfQuery;
    let as_of = || "2026-07-01T00:00:00.000Z".to_string();
    let both = AsOfQuery::from_parts(as_of(), Some(RecordType::Source), Some(vec![RecordType::Source]), None);
    assert_eq!(both.unwrap_err().to_string(), "Use record_type or record_types, not both");
    let empty = AsOfQuery::from_parts(as_of(), None, Some(Vec::new()), None);
    assert_eq!(empty.unwrap_err().to_string(), "record_types must contain at least one record type");
    let one = AsOfQuery::from_parts(as_of(), Some(RecordType::Source), None, None).unwrap();
    assert_eq!(one.record_types, vec![RecordType::Source]);
}
