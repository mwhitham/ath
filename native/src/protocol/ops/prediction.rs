//! Log planning, attachments, grading, evidence, and backtests.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::json::{self, Map, Value};
use crate::log::{LogPlan, LogRecord, LogRequest};
use crate::model::time;
use crate::model::{CurrentRecord, EvidenceReference, RecordActor, RecordData, RecordType};
use crate::prediction::grade::{GradeRequest, PlannedCorrection, PlannedRecord};
use crate::prediction::{BacktestOptions, EvidenceRequest};
use crate::protocol::definition::{Callback, Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};
use crate::runtime::keyring::Gateway;
use crate::runtime::model_client::{ModelError, ModelReply, PromptRequest};

use super::input;

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "log.plan",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ text: unknown; actor: unknown; created_at?: unknown; benchmark?: unknown }",
        output: "Api.LogPlan",
        items: None,
        callbacks: &[],
        run: log_plan,
    },
    Operation {
        name: "log.add_attachments",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ plan: unknown; attachments: readonly unknown[] }",
        output: "Api.LogPlan",
        items: None,
        callbacks: &[],
        run: add_attachments,
    },
    Operation {
        name: "attachments.prepare",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ paths: readonly string[]; actor: unknown; created_at: unknown }",
        output: "Array<{ input: Api.WriteRecordInput; bytes_base64: string; final_path: string }>",
        items: None,
        callbacks: &[],
        run: prepare_attachments,
    },
    Operation {
        name: "grade.plan",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ prediction_id: unknown; result_id: unknown; actor: unknown; calculation_version?: unknown; created_at?: unknown }",
        output: "Api.PredictionGradePlan",
        items: None,
        callbacks: &[],
        run: grade_plan,
    },
    Operation {
        name: "evidence.build",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ benchmark: unknown; as_of?: unknown; limit?: unknown; window_days?: unknown }",
        output: "Api.PredictionEvidence",
        items: None,
        callbacks: &[],
        run: evidence,
    },
    Operation {
        name: "evidence.resolve_reference",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ reference: unknown }",
        output: "Api.EvidenceReferenceResolution",
        items: None,
        callbacks: &[],
        run: resolve_reference,
    },
    Operation {
        name: "backtest.run",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ models: readonly string[]; gateway: string; created_at?: string }",
        output: "Api.V1BacktestReport",
        items: None,
        callbacks: &[Callback {
            name: "complete",
            input: "Api.CompleteRequest",
            output: "Api.CompleteResponse",
        }],
        run: backtest,
    },
    Operation {
        name: "backtest.write_report",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ database_path: string; report: unknown }",
        output: "string",
        items: None,
        callbacks: &[],
        run: write_report,
    },
];

fn actor(value: Option<&Value>) -> Result<RecordActor, Failure> {
    input::decode(value.unwrap_or(&Value::Null))
}

/// A planned record in the 2.1.0 `WriteRecordInput` key order.
fn write_input(
    record_type: RecordType,
    record_id: &str,
    created_at: &str,
    changed_by: &RecordActor,
    data: &RecordData,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("record_type".into(), Value::String(record_type.as_str().into()));
    out.insert("record_id".into(), Value::String(record_id.to_string()));
    out.insert("created_at".into(), Value::String(created_at.to_string()));
    out.insert("changed_by".into(), json::to_value(changed_by));
    out.insert("data".into(), data.to_value());
    out
}

fn log_record_value(record: &LogRecord) -> Value {
    let mut out =
        write_input(record.record_type(), &record.record_id, &record.created_at, &record.changed_by, &record.data);
    if let Some(extensions) = &record.extensions {
        out.insert("extensions".into(), Value::Object(extensions.clone()));
    }
    Value::Object(out)
}

fn plan_value(plan: &LogPlan) -> Value {
    serde_json::json!({
        "inputs": plan.inputs.iter().map(log_record_value).collect::<Vec<_>>(),
        "expected_current_revisions": plan
            .expected_current_revisions
            .iter()
            .map(|(record_id, revision)| serde_json::json!({ "record_id": record_id, "revision": revision }))
            .collect::<Vec<_>>(),
        "interpretation": plan.interpretation,
    })
}

/// A `WriteRecordInput` a caller passes back, checked as a current record.
fn log_record(value: &Value) -> Result<LogRecord, Failure> {
    let record_id =
        input::checked_text_or("RecordId", value.get("record_id").filter(|v| !v.is_null()), crate::ids::new_record_id)?;
    let created_at =
        input::checked_text_or("UtcTimestamp", value.get("created_at").filter(|v| !v.is_null()), time::now_utc)?;
    let mut candidate = Map::new();
    candidate.insert("record_id".into(), Value::String(record_id.clone()));
    candidate.insert("record_type".into(), value.get("record_type").cloned().unwrap_or(Value::Null));
    candidate.insert("created_at".into(), Value::String(created_at.clone()));
    candidate.insert("current_revision".into(), Value::from(1));
    candidate.insert("is_deleted".into(), Value::Bool(false));
    if let Some(data) = value.get("data") {
        candidate.insert("data".into(), data.clone());
    }
    if let Some(extensions) = value.get("extensions").filter(|v| !v.is_null()) {
        candidate.insert("extensions".into(), extensions.clone());
    }
    let record = CurrentRecord::from_value(&Value::Object(candidate))?;
    Ok(LogRecord {
        record_id,
        created_at,
        changed_by: actor(value.get("changed_by"))?,
        data: record.data,
        extensions: record.extensions,
    })
}

fn plan_from(value: &Value) -> Result<LogPlan, Failure> {
    let inputs = input::array(value, "inputs")?.iter().map(log_record).collect::<Result<Vec<_>, _>>()?;
    let expected = input::array(value, "expected_current_revisions")?
        .iter()
        .map(|expected| {
            let record_id = expected.get("record_id").and_then(Value::as_str).unwrap_or_default().to_string();
            let revision = expected.get("revision").and_then(input::whole_number).unwrap_or(0);
            (record_id, revision)
        })
        .collect();
    let interpretation = value.get("interpretation").and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(LogPlan { inputs, expected_current_revisions: expected, interpretation })
}

/// `planLogEntry(database, input)`: plain text or structured records, with
/// unambiguous automatic grades.
fn log_plan(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let created_at = match input::get(&request, "created_at") {
        Some(value) => Some(input::checked_text("UtcTimestamp", value)?),
        None => None,
    };
    let text = match request.get("text") {
        Some(Value::String(text)) => text.clone(),
        _ => return Err(Error::other("input.text.trim is not a function").into()),
    };
    let log = LogRequest {
        text,
        actor: actor(request.get("actor"))?,
        created_at,
        benchmark: input::string(&request, "benchmark")?,
    };
    let plan = crate::log::plan_log_entry(call.store()?.connection(), &log, &mut crate::ids::new_record_id)?;
    Ok(plan_value(&plan))
}

/// `addAttachmentsToLogPlan(plan, attachments)`.
fn add_attachments(call: &mut Call<'_>) -> Outcome {
    let plan = plan_from(input::required(&call.input, "plan")?)?;
    let attachments =
        input::array(&call.input, "attachments")?.iter().map(log_record).collect::<Result<Vec<_>, _>>()?;
    Ok(plan_value(&crate::log::add_attachments(plan, attachments)?))
}

/// `prepareAttachments(database, paths, actor, createdAt)`: each file read and
/// checked; nothing is written.
fn prepare_attachments(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let paths: Vec<PathBuf> = input::array(&request, "paths")?
        .iter()
        .map(|path| path.as_str().map(PathBuf::from).ok_or_else(|| Failure::Input("paths must be strings".into())))
        .collect::<Result<_, _>>()?;
    let changed_by = actor(request.get("actor"))?;
    let created_at = request.get("created_at").and_then(Value::as_str).unwrap_or_default().to_string();
    let prepared = crate::package::attachments::prepare_attachments(call.store()?, &paths, &changed_by, &created_at)?;
    let out: Vec<Value> = prepared
        .iter()
        .map(|attachment| {
            let mut record = Map::new();
            record.insert("record_id".into(), Value::String(attachment.record_id.clone()));
            record.insert("record_type".into(), Value::String("attachment".into()));
            record.insert("created_at".into(), Value::String(attachment.created_at.clone()));
            record.insert("changed_by".into(), json::to_value(&attachment.changed_by));
            record.insert("data".into(), RecordData::Attachment(attachment.data.clone()).to_value());
            serde_json::json!({
                "input": Value::Object(record),
                "bytes_base64": base64(attachment.bytes()),
                "final_path": attachment.final_path.display().to_string(),
            })
        })
        .collect();
    Ok(Value::Array(out))
}

/// `planPredictionGrade(database, input)`.
fn grade_plan(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let grade = GradeRequest {
        prediction_id: input::checked_text("RecordId", request.get("prediction_id").unwrap_or(&Value::Null))?,
        result_id: input::checked_text("RecordId", request.get("result_id").unwrap_or(&Value::Null))?,
        actor: actor(request.get("actor"))?,
        calculation_version: input::string(&request, "calculation_version")?,
        created_at: match input::get(&request, "created_at") {
            Some(value) => Some(input::checked_text("UtcTimestamp", value)?),
            None => None,
        },
    };
    let plan = crate::prediction::grade::plan_prediction_grade(
        call.store()?.connection(),
        &grade,
        &mut crate::ids::new_record_id,
    )?;
    let planned = |record: &PlannedRecord| {
        Value::Object(write_input(
            record.data.record_type(),
            &record.record_id,
            &record.created_at,
            &record.changed_by,
            &record.data,
        ))
    };
    let correction = |correction: &PlannedCorrection| {
        serde_json::json!({
            "record_id": correction.record_id,
            "expected_revision": correction.expected_revision,
            "changed_by": json::to_value(&correction.changed_by),
            "became_known_at": correction.became_known_at,
            "data": correction.data.to_value(),
        })
    };
    Ok(serde_json::json!({
        "prediction": plan.prediction.to_value(),
        "result": plan.result.to_value(),
        "existing_link": plan.existing_link.as_ref().map(CurrentRecord::to_value),
        "link_input": plan.link_input.as_ref().map(planned),
        "link_correction": plan.link_correction.as_ref().map(correction),
        "grade_input": planned(&plan.grade_input),
    }))
}

/// `buildEvidence(database, input)`: offline evidence, not a prediction.
fn evidence(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let benchmark = match request.get("benchmark") {
        Some(Value::String(text)) => text.clone(),
        _ => return Err(Failure::Input("benchmark must be a string".into())),
    };
    let as_of = match input::get(&request, "as_of") {
        Some(value) => input::checked_text("UtcTimestamp", value)?,
        None => time::now_utc(),
    };
    let number = |key: &str| input::get(&request, key).map(|value| input::whole_number(value).map_or(0, |n| n as u32));
    let evidence_request =
        EvidenceRequest { benchmark, as_of, limit: number("limit"), window_days: number("window_days") };
    let evidence = crate::prediction::build_evidence(call.store()?.connection(), &evidence_request)?;
    Ok(json::to_value(&evidence))
}

/// `resolveEvidenceReference(database, reference)`.
fn resolve_reference(call: &mut Call<'_>) -> Outcome {
    let reference: EvidenceReference = input::decode(call.input.get("reference").unwrap_or(&Value::Null))?;
    let resolved = crate::prediction::resolve_evidence_reference(call.store()?.connection(), &reference)?;
    Ok(json::to_value(&resolved))
}

/// `runV1Backtest(database, input)`: each model completion is a callback to
/// the caller's `complete` function. Athlete data is not changed.
fn backtest(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let models: Vec<String> = input::array(&request, "models")?
        .iter()
        .map(|model| model.as_str().map(str::to_string).ok_or_else(|| Failure::Input("models must be strings".into())))
        .collect::<Result<_, _>>()?;
    let gateway_text = input::required_string(&request, "gateway")?;
    let gateway =
        Gateway::from_name(&gateway_text).ok_or_else(|| Error::other(format!("Unknown gateway: {gateway_text}")))?;
    let options = BacktestOptions { models, created_at: input::string(&request, "created_at")? };
    let (store, callbacks) = call.store_and_callbacks()?;
    // The caller's `complete` receives the 2.1.0 request shape:
    // `{ gateway, model, system, user }`.
    let complete = |prompt: &PromptRequest<'_>| -> Result<ModelReply, ModelError> {
        let completion = serde_json::json!({
            "gateway": json::to_value(&gateway),
            "model": prompt.model,
            "system": prompt.system,
            "user": prompt.user,
        });
        match callbacks.call("complete", completion).map_err(|error| ModelError::Transport(error.to_string()))? {
            Ok(answer) => {
                let content = answer.get("content").and_then(Value::as_str);
                let model = answer.get("model").and_then(Value::as_str);
                match (content, model) {
                    (Some(content), Some(model)) => {
                        Ok(ModelReply { content: content.to_string(), model: model.to_string() })
                    }
                    _ => Err(ModelError::Transport("complete must resolve to { content, model } strings".into())),
                }
            }
            Err(message) => Err(ModelError::Transport(message)),
        }
    };
    let report = crate::prediction::run_backtest(store.connection(), &options, &complete)?;
    Ok(json::to_value(&report))
}

/// `writeV1BacktestReport(databasePath, report)`: a new owner-only file
/// beside the database, named from the report time.
fn write_report(call: &mut Call<'_>) -> Outcome {
    let database_path = input::required_string(&call.input, "database_path")?;
    if is_remote_url(&database_path) {
        return Err(Error::refused(format!(
            "A backtest report is written beside a local database file, not a remote URL: {database_path}"
        ))
        .into());
    }
    let report = input::required(&call.input, "report")?;
    let created_at = report.get("created_at").map(json::to_string).map(|text| text.trim_matches('"').to_string());
    let created_at = created_at.unwrap_or_else(|| "undefined".into());
    // The path keeps the caller's form, as 2.1.0's `join(dirname(path), name)` did.
    let path = crate::prediction::backtest::report_path(Path::new(&database_path), &created_at);
    let mut text = json::to_pretty(report);
    text.push('\n');
    crate::files::write_new_synced(&path, text.as_bytes())?;
    Ok(Value::String(path.display().to_string()))
}

/// A URL with a scheme other than `file:`, as 2.1.0's target parse read it.
pub(crate) fn is_remote_url(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else { return false };
    rest.starts_with("//")
        && !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
        && !scheme.eq_ignore_ascii_case("file")
}

/// Standard base64 with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Decode standard base64 with padding.
pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |byte: u8| -> Option<u32> {
        Some(match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let padding = chunk.iter().rev().take_while(|byte| **byte == b'=').count();
        if padding > 2 {
            return None;
        }
        let mut n = 0u32;
        for (index, byte) in chunk.iter().enumerate() {
            let digit = if index >= 4 - padding { 0 } else { value(*byte)? };
            n = (n << 6) | digit;
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_round_trips() {
        for text in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            let encoded = super::base64(text.as_bytes());
            assert_eq!(super::base64_decode(&encoded).unwrap(), text.as_bytes());
        }
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(super::base64(b"fo"), "Zm8=");
    }

    #[test]
    fn remote_urls_are_recognized_like_the_old_parse() {
        assert!(super::is_remote_url("libsql://example.turso.io"));
        assert!(super::is_remote_url("HTTP://host/db"));
        assert!(!super::is_remote_url("file:///tmp/a.ath.sqlite"));
        assert!(!super::is_remote_url("C:\\data\\a.ath.sqlite"));
        assert!(!super::is_remote_url("./a.ath.sqlite"));
    }
}
