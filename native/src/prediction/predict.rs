//! `ath predict` with a model: ask, check the reply, preview, then write.
//!
//! The model call happens before any write. A network failure, a refusal, or
//! a reply that does not validate returns an error and writes nothing. The
//! saved prediction is one record written in one transaction.

use serde::Serialize;

use super::client::{CompleteRequest, Gateway, ModelClient};
use super::evidence::PredictionEvidence;
use crate::error::{Error, Result};
use crate::json::{self, Map, Value};
use crate::model::*;
use crate::store::{Changes, NewRecord, Store};

/// The agent name the terminal prediction path writes under.
pub const TERMINAL_AGENT: &str = "ath-terminal";

pub const PREDICT_SYSTEM_PROMPT: &str =
    "Return only JSON with predicted_score, optional predicted_range, confidence, and reasoning. \
Use the benchmark score fields duration_s, reps, or weight_kg. Do not add medical advice.";

/// Fields the program sets on a prediction. A model reply cannot set them.
pub const TOOL_SET_FIELDS: &[&str] = &["authorship", "benchmark_definition_id"];

/// Strip one surrounding code fence, as models often add one.
pub fn strip_code_fence(content: &str) -> &str {
    let mut text = content.trim();
    if let Some(rest) = text.strip_prefix("```") {
        text = rest;
        if text.get(..4).is_some_and(|word| word.eq_ignore_ascii_case("json")) {
            text = &text[4..];
        }
        text = text.trim_start();
    }
    if let Some(stripped) = text.strip_suffix("```") {
        text = stripped.trim_end();
    }
    text
}

/// Read a model reply as one JSON object. A reply that tries to set the
/// authorship or benchmark is refused, so a model cannot claim to be the
/// athlete or move its prediction to another benchmark.
pub fn parse_prediction_response(content: &str) -> Result<Map<String, Value>> {
    let value = json::parse(strip_code_fence(content))
        .map_err(|error| Error::other(format!("model did not return valid prediction JSON: {}", error.message)))?;
    let Value::Object(map) = value else {
        return Err(Error::other("model prediction response must be one JSON object"));
    };
    if let Some(field) = TOOL_SET_FIELDS.iter().find(|field| map.contains_key(**field)) {
        return Err(Error::refused(format!(
            "model prediction response cannot set {field}; ath sets the authorship and benchmark"
        )));
    }
    Ok(map)
}

/// A prediction ready to preview and save.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionPreview {
    /// The benchmark key.
    pub benchmark: String,
    /// The model ID the gateway reported.
    pub model: String,
    #[serde(serialize_with = "serialize_prediction")]
    pub data: PredictionData,
    /// Always false: a preview never writes.
    pub write_confirmed: bool,
}

fn serialize_prediction<S: serde::Serializer>(
    data: &PredictionData,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    json::to_value(data).serialize(serializer)
}

/// Build prediction data from evidence and a checked model reply.
pub fn prediction_data(
    evidence: &PredictionEvidence,
    model: &str,
    reply: Map<String, Value>,
    authored_at: &str,
) -> Result<PredictionData> {
    let author = AuthoredRecordAuthor::Agent {
        agent: TERMINAL_AGENT.into(),
        agent_version: Some(ATHLETIC_STANDARD_VERSION.into()),
        model: model.into(),
        model_version: None,
    };
    let authorship = AuthoredRecordMetadata {
        author,
        authored_at: authored_at.into(),
        evidence: evidence.references(),
        content_version: 1,
        status: AuthoredRecordStatus::Active,
    };
    let mut data = Map::new();
    data.insert("authorship".into(), json::to_value(&authorship));
    data.insert("benchmark_definition_id".into(), Value::String(evidence.benchmark.record_id.clone()));
    for (key, value) in reply {
        data.insert(key, value);
    }
    let RecordData::Prediction(prediction) = RecordData::decode(RecordType::Prediction, &Value::Object(data))? else {
        unreachable!("decode returns the requested type")
    };
    Ok(prediction)
}

/// Ask a model to predict from evidence and check its reply. Writes nothing.
pub fn request_prediction(
    evidence: &PredictionEvidence,
    client: &mut dyn ModelClient,
    gateway: Gateway,
    model: &str,
    authored_at: &str,
) -> Result<PredictionPreview> {
    let response = client.complete(&CompleteRequest {
        gateway,
        model: model.to_string(),
        system: PREDICT_SYSTEM_PROMPT.to_string(),
        user: evidence.model_text(),
    })?;
    let reply = parse_prediction_response(&response.content)?;
    let data = prediction_data(evidence, &response.model, reply, authored_at)?;
    let RecordData::BenchmarkDefinition(definition) = &evidence.benchmark.data else {
        return Err(Error::other("evidence benchmark is not a benchmark definition"));
    };
    Ok(PredictionPreview {
        benchmark: definition.benchmark_key.clone(),
        model: response.model,
        data,
        write_confirmed: false,
    })
}

/// The saved prediction.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionWritten {
    #[serde(serialize_with = "crate::view::serialize_record")]
    pub record: CurrentRecord,
    pub database_revision: u64,
}

/// Save a previewed prediction as one record authored by the terminal agent.
pub fn write_prediction(store: &mut Store, preview: &PredictionPreview) -> Result<PredictionWritten> {
    let changed_by =
        RecordActor::Agent { agent: TERMINAL_AGENT.into(), agent_version: Some(ATHLETIC_STANDARD_VERSION.into()) };
    let mut record = NewRecord::new(RecordData::Prediction(preview.data.clone()), changed_by);
    record.created_at = Some(preview.data.authorship.authored_at.clone());
    let applied = store.apply(Changes { creates: vec![record], ..Changes::default() })?;
    let record =
        applied.records.into_iter().next().ok_or_else(|| Error::other("prediction write returned no record"))?;
    Ok(PredictionWritten { record, database_revision: applied.database_revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_are_stripped() {
        assert_eq!(strip_code_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_code_fence("```JSON {\"a\":1}```"), "{\"a\":1}");
        assert_eq!(strip_code_fence("  {\"a\":1}  "), "{\"a\":1}");
    }

    #[test]
    fn replies_must_be_one_object_and_cannot_set_authorship() {
        assert_eq!(
            parse_prediction_response("[1]").unwrap_err().to_string(),
            "model prediction response must be one JSON object"
        );
        assert!(parse_prediction_response("nope")
            .unwrap_err()
            .to_string()
            .starts_with("model did not return valid prediction JSON: "));
        let claim = r#"{"authorship":{"author":{"author_type":"athlete"}},"predicted_score":{"duration_s":280}}"#;
        assert_eq!(
            parse_prediction_response(claim).unwrap_err().to_string(),
            "model prediction response cannot set authorship; ath sets the authorship and benchmark"
        );
        let moved = r#"{"benchmark_definition_id":"01890f3e-e7d3-7cc0-88c8-000000000006"}"#;
        assert!(parse_prediction_response(moved).unwrap_err().to_string().contains("benchmark_definition_id"));
    }
}
