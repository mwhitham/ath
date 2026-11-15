//! One OpenAI-compatible client for Vercel AI Gateway and OpenRouter (D75, D78).
//!
//! `GET /models` gives the live list: every text model the catalog marks as
//! able to reason, including open weights. `POST /chat/completions` calls one
//! model. There are no per-provider clients. Tests point the base URL at a
//! local server; automated checks never call a live gateway.

use std::io::Read;
use std::time::Duration;

use serde::Serialize;

use super::collate::locale_compare;
use super::keyring::{Gateway, Keyring};
use super::model_client::{ModelClient, ModelError, ModelReply, PromptRequest};
use crate::json::{self, Map, Value};

/// The Vercel AI Gateway API root.
pub const VERCEL_BASE_URL: &str = "https://ai-gateway.vercel.sh/v1";

/// The OpenRouter API root.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// The largest gateway reply read. A larger reply is an error, not a
/// shortened list.
pub const MAX_REPLY_BYTES: u64 = 64 * 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(300);

/// The API root for each gateway. Tests replace them with a local server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayEndpoints {
    pub vercel: String,
    pub openrouter: String,
}

impl Default for GatewayEndpoints {
    fn default() -> Self {
        GatewayEndpoints { vercel: VERCEL_BASE_URL.to_string(), openrouter: OPENROUTER_BASE_URL.to_string() }
    }
}

impl GatewayEndpoints {
    /// Both gateways at one root, for a test server.
    pub fn both(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into();
        GatewayEndpoints { vercel: base_url.clone(), openrouter: base_url }
    }

    pub fn base(&self, gateway: Gateway) -> &str {
        match gateway {
            Gateway::Vercel => &self.vercel,
            Gateway::OpenRouter => &self.openrouter,
        }
    }
}

/// One entry on the live model list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl CatalogModel {
    pub fn new(id: impl Into<String>) -> Self {
        CatalogModel { id: id.into(), name: None }
    }
}

/// A connection to one gateway with one key. Creating it reads nothing and
/// sends nothing.
pub struct GatewayClient {
    gateway: Gateway,
    key: String,
    base_url: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GatewayClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayClient")
            .field("gateway", &self.gateway)
            .field("base_url", &self.base_url)
            .field("key", &"<not shown>")
            .finish()
    }
}

impl GatewayClient {
    pub fn new(gateway: Gateway, key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout_read(READ_TIMEOUT)
            .timeout_write(READ_TIMEOUT)
            .build();
        GatewayClient { gateway, key: key.into(), base_url: base_url.into(), agent }
    }

    /// The gateway and key to use, or a refusal that names the remedy.
    ///
    /// A key in the environment wins over a saved key. When both gateways
    /// have keys and none was named, this refuses instead of guessing.
    pub fn connect(
        keyring: &Keyring<'_>,
        requested: Option<Gateway>,
        endpoints: &GatewayEndpoints,
    ) -> Result<GatewayClient, ModelError> {
        let gateway = keyring.resolve_gateway(requested)?.ok_or(ModelError::NoKey)?;
        let key = keyring.resolve_key(gateway).ok_or(ModelError::NoKey)?;
        Ok(GatewayClient::new(gateway, key, endpoints.base(gateway)))
    }

    pub fn gateway(&self) -> Gateway {
        self.gateway
    }

    /// The live list of text models that can reason, sorted by ID.
    pub fn list_reasoning_models(&self) -> Result<Vec<CatalogModel>, ModelError> {
        let request = self.with_headers(self.agent.get(&format!("{}/models", self.base_url)));
        let body = self.read_json(request.call())?;
        Ok(reasoning_models_from_catalog(&body))
    }

    fn with_headers(&self, request: ureq::Request) -> ureq::Request {
        let request = request.set("authorization", &format!("Bearer {}", self.key));
        match self.gateway {
            Gateway::OpenRouter => {
                request.set("http-referer", "https://athleticstandard.ai").set("x-title", "Athletic Standard")
            }
            Gateway::Vercel => request,
        }
    }

    fn read_json(&self, result: Result<ureq::Response, ureq::Error>) -> Result<Value, ModelError> {
        let response = match result {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(transport)) => return Err(ModelError::Transport(transport.to_string())),
        };
        let status = response.status();
        let status_text = response.status_text().to_string();
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_REPLY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| ModelError::Transport(error.to_string()))?;
        if bytes.len() as u64 > MAX_REPLY_BYTES {
            return Err(ModelError::Transport(format!(
                "the {} gateway reply is larger than {} MiB",
                self.gateway,
                MAX_REPLY_BYTES / 1024 / 1024
            )));
        }
        // Some error pages are not JSON. Their body counts as an empty object.
        let body = match json::parse_bytes(&bytes) {
            Ok(Value::Null) | Err(_) => Value::Object(Map::new()),
            Ok(value) => value,
        };
        if !(200..300).contains(&status) {
            return Err(refusal(self.gateway, status, &status_text, &body));
        }
        Ok(body)
    }
}

impl ModelClient for GatewayClient {
    fn complete(&self, request: &PromptRequest<'_>) -> Result<ModelReply, ModelError> {
        let body = chat_request_body(request);
        let http = self
            .with_headers(self.agent.post(&format!("{}/chat/completions", self.base_url)))
            .set("content-type", "application/json");
        let reply = self.read_json(http.send_string(&json::to_string(&body)))?;
        let Some(content) = content_from(&reply) else {
            return Err(ModelError::NoText { gateway: self.gateway, model: request.model.to_string() });
        };
        let model = match reply.get("model") {
            Some(Value::String(model)) => model.clone(),
            _ => request.model.to_string(),
        };
        Ok(ModelReply { content, model })
    }
}

/// The `POST /chat/completions` body. Without a response shape it is the
/// body 2.1.0 sent, key for key.
pub fn chat_request_body(request: &PromptRequest<'_>) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), Value::String(request.model.to_string()));
    body.insert("temperature".into(), Value::from(0));
    body.insert("max_tokens".into(), Value::from(8192));
    body.insert(
        "messages".into(),
        Value::Array(vec![message("system", request.system), message("user", request.user)]),
    );
    if let Some(shape) = request.response_shape {
        let mut schema = Map::new();
        schema.insert("name".into(), Value::String("response".into()));
        schema.insert("schema".into(), shape.clone());
        let mut format = Map::new();
        format.insert("type".into(), Value::String("json_schema".into()));
        format.insert("json_schema".into(), Value::Object(schema));
        body.insert("response_format".into(), Value::Object(format));
    }
    Value::Object(body)
}

fn message(role: &str, content: &str) -> Value {
    let mut message = Map::new();
    message.insert("role".into(), Value::String(role.to_string()));
    message.insert("content".into(), Value::String(content.to_string()));
    Value::Object(message)
}

fn refusal(gateway: Gateway, status: u16, status_text: &str, body: &Value) -> ModelError {
    let detail = match body.get("error") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(error)) if error.contains_key("message") => js_string(&error["message"]),
        _ => status_text.to_string(),
    };
    let mut message = format!("the {gateway} gateway refused the call ({status}). {detail}").trim().to_string();
    if status == 401 || status == 403 {
        message.push_str(" Check the key with `ath key`.");
    }
    ModelError::GatewayRefused { gateway, status, message }
}

/// The reply text: the first choice's message content, or its `text`.
pub fn content_from(body: &Value) -> Option<String> {
    let first = match body.get("choices") {
        Some(Value::Array(choices)) => choices.first()?,
        _ => return None,
    };
    if let Some(Value::Object(message)) = first.get("message") {
        if let Some(Value::String(content)) = message.get("content") {
            if !content.trim().is_empty() {
                return Some(content.clone());
            }
        }
    }
    match first.get("text") {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => None,
    }
}

/// The text models that can reason, from a `GET /models` reply, sorted by ID
/// in JavaScript `localeCompare` order.
pub fn reasoning_models_from_catalog(body: &Value) -> Vec<CatalogModel> {
    let items: &[Value] = match body {
        Value::Array(items) => items,
        Value::Object(object) => match object.get("data") {
            Some(Value::Array(items)) => items,
            _ => &[],
        },
        _ => &[],
    };
    let mut models = Vec::new();
    for item in items {
        let Value::Object(record) = item else { continue };
        let Some(Value::String(id)) = record.get("id") else { continue };
        if id.is_empty() || !is_reasoning_text_model(item) {
            continue;
        }
        let name = match record.get("name") {
            Some(Value::String(name)) if !name.is_empty() => Some(name.clone()),
            _ => None,
        };
        models.push(CatalogModel { id: id.clone(), name });
    }
    models.sort_by(|left, right| locale_compare(&left.id, &right.id));
    models
}

const NON_TEXT_TYPES: [&str; 10] =
    ["embedding", "embeddings", "image", "audio", "transcription", "tts", "moderation", "rerank", "video", "vision"];

const NON_TEXT_MODALITIES: [&str; 4] = ["image", "audio", "video", "embedding"];

const NON_TEXT_ID_PARTS: [&str; 9] =
    ["embedding", "whisper", "tts", "dall-e", "imagen", "moderation", "rerank", "wav", "audio"];

/// Text in, text out, and the catalog marks the model as able to reason (D78).
/// Image, audio, and embedding models are out. There is no favourite-labs list.
pub fn is_reasoning_text_model(raw: &Value) -> bool {
    let id = string_field(raw, "id").map(|id| id.to_lowercase()).unwrap_or_default();
    let kind = string_field(raw, "type").map(|kind| kind.to_lowercase()).unwrap_or_default();
    if NON_TEXT_TYPES.contains(&kind.as_str()) {
        return false;
    }
    if NON_TEXT_ID_PARTS.iter().any(|part| id.contains(part)) {
        return false;
    }

    let architecture = match raw.get("architecture") {
        Some(value @ (Value::Object(_) | Value::Array(_))) => Some(value),
        _ => None,
    };
    let field = |first: &str, second: &str| {
        architecture.and_then(|value| match value.get(first) {
            None | Some(Value::Null) => value.get(second),
            found => found,
        })
    };
    let input = modalities(field("input_modalities", "input_modality"));
    let output = modalities(field("output_modalities", "output_modality"));
    let modality = architecture.and_then(|value| string_field(value, "modality")).map(|text| text.to_lowercase());

    let has_text = |list: &[String]| list.iter().any(|item| item == "text");
    if !input.is_empty() && !has_text(&input) {
        return false;
    }
    if !output.is_empty() && !has_text(&output) {
        return false;
    }
    if let Some(modality) = modality {
        if !modality.is_empty() && !modality.contains("text") {
            return false;
        }
    }
    let non_text = |list: &[String]| list.iter().any(|item| NON_TEXT_MODALITIES.contains(&item.as_str()));
    if non_text(&input) && !has_text(&input) {
        return false;
    }
    if non_text(&output) {
        return false;
    }
    has_reasoning_mark(raw)
}

fn string_field<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    match value.get(name) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

fn modalities(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.to_lowercase()),
                _ => None,
            })
            .collect(),
        Some(Value::String(text)) => text
            .to_lowercase()
            .split(|character: char| !character.is_ascii_lowercase())
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn has_reasoning_mark(raw: &Value) -> bool {
    if let Some(Value::Bool(true) | Value::Object(_) | Value::Array(_)) = raw.get("reasoning") {
        return true;
    }
    if let Some(capabilities @ (Value::Object(_) | Value::Array(_))) = raw.get("capabilities") {
        if capabilities.get("reasoning") == Some(&Value::Bool(true)) {
            return true;
        }
    }
    let lowered = |name: &str| -> Vec<String> {
        match raw.get(name) {
            Some(Value::Array(items)) => items.iter().map(|item| js_string(item).to_lowercase()).collect(),
            _ => Vec::new(),
        }
    };
    if lowered("tags").iter().any(|tag| tag.contains("reasoning")) {
        return true;
    }
    lowered("supported_parameters")
        .iter()
        .any(|parameter| matches!(parameter.as_str(), "reasoning" | "include_reasoning" | "reasoning_effort"))
}

/// JavaScript's `String(value)` for JSON values.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(_) => json::to_string(value),
        Value::Array(items) => items
            .iter()
            .map(|item| if item.is_null() { String::new() } else { js_string(item) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// The closest live IDs to a name that is not on the list, so a typo gives
/// choices rather than a dead end.
pub fn closest_model_ids(wanted: &str, catalog: &[CatalogModel], n: usize) -> Vec<String> {
    let needle = wanted.to_lowercase();
    let mut scored: Vec<(&str, usize)> = catalog
        .iter()
        .map(|model| (model.id.as_str(), closeness(&needle, &model.id.to_lowercase())))
        .filter(|(_, score)| *score > 0)
        .collect();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scored.into_iter().take(n).map(|(id, _)| id.to_string()).collect()
}

fn closeness(a: &str, b: &str) -> usize {
    let shared = a.encode_utf16().zip(b.encode_utf16()).take_while(|(left, right)| left == right).count();
    let contains = if a.contains(b) || b.contains(a) { 3 } else { 0 };
    let tail = match a.find('/') {
        Some(slash) if b.ends_with(&a[slash + 1..]) => 4,
        _ => 0,
    };
    shared + contains + tail
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> Value {
        json::parse(text).unwrap()
    }

    #[test]
    fn string_conversion_matches_javascript() {
        assert_eq!(js_string(&value("[1,null,\"a\",[2,3]]")), "1,,a,2,3");
        assert_eq!(js_string(&value("{}")), "[object Object]");
        assert_eq!(js_string(&value("1.50")), "1.5");
    }

    #[test]
    fn reply_text_comes_from_message_or_text() {
        assert_eq!(
            content_from(&value(r#"{"choices":[{"message":{"content":" {\"a\":1} "}}]}"#)).as_deref(),
            Some(" {\"a\":1} ")
        );
        assert_eq!(content_from(&value(r#"{"choices":[{"text":"  hi  "}]}"#)).as_deref(), Some("hi"));
        assert_eq!(content_from(&value(r#"{"choices":[{"message":{"content":"   "}}]}"#)), None);
        assert_eq!(content_from(&value(r#"{"choices":[]}"#)), None);
        assert_eq!(content_from(&value(r#"{}"#)), None);
    }
}
