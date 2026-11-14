//! The gateway client: the live model list, model calls, and refusals.
//! Ported from `tests/gateway.test.ts`; the HTTP cases use a local server.

mod support;

use athleticstandard::json;
use athleticstandard::runtime::gateway::{
    chat_request_body, closest_model_ids, is_reasoning_text_model, reasoning_models_from_catalog, CatalogModel,
    GatewayClient, GatewayEndpoints,
};
use athleticstandard::runtime::keyring::{Gateway, Keyring, MemorySecretStore, SecretStore};
use athleticstandard::runtime::model_client::{no_key_message, ModelClient, ModelError, ModelReply, PromptRequest};
use support::{closed_port_url, Reply, Server};

fn value(text: &str) -> json::Value {
    json::parse(text).unwrap()
}

fn request<'a>(model: &'a str) -> PromptRequest<'a> {
    PromptRequest { model, system: "sys", user: "{\"a\":1}", response_shape: None }
}

#[test]
fn keeps_a_language_model_the_catalog_marks_as_reasoning() {
    assert!(is_reasoning_text_model(&value(r#"{"id":"qwen/qwen3-235b-a22b","type":"language","tags":["reasoning"]}"#)));
    assert!(is_reasoning_text_model(&value(
        r#"{"id":"openai/gpt-oss-120b","architecture":{"input_modalities":["text"],"output_modalities":["text"]},
            "supported_parameters":["include_reasoning","reasoning"]}"#
    )));
    assert!(is_reasoning_text_model(&value(r#"{"id":"a/b","reasoning":{}}"#)));
    assert!(is_reasoning_text_model(&value(r#"{"id":"a/b","capabilities":{"reasoning":true}}"#)));
    assert!(is_reasoning_text_model(&value(
        r#"{"id":"a/b","architecture":{"modality":"text->text"},"tags":["Reasoning-Effort"]}"#
    )));
}

#[test]
fn drops_embeddings_images_and_chat_models_with_no_reasoning_mark() {
    assert!(!is_reasoning_text_model(&value(r#"{"id":"openai/text-embedding-3-large","type":"embedding"}"#)));
    assert!(!is_reasoning_text_model(&value(
        r#"{"id":"openai/gpt-4o","architecture":{"input_modalities":["text","image"],"output_modalities":["text"]},
            "supported_parameters":["temperature"]}"#
    )));
    assert!(!is_reasoning_text_model(&value(
        r#"{"id":"black-forest-labs/flux","architecture":{"output_modalities":["image"]},"tags":["reasoning"]}"#
    )));
    assert!(!is_reasoning_text_model(&value(r#"{"id":"openai/whisper-1","reasoning":true}"#)));
    assert!(!is_reasoning_text_model(&value(
        r#"{"id":"x/y","architecture":{"modality":"image->image"},"reasoning":true}"#
    )));
    assert!(!is_reasoning_text_model(&value(r#"{"id":"x/y","reasoning":false}"#)));
}

#[test]
fn names_close_models_when_the_one_asked_for_is_not_on_the_list() {
    let catalog: Vec<CatalogModel> = ["qwen/qwen3-235b-a22b", "openai/gpt-oss-120b", "moonshotai/kimi-k2"]
        .into_iter()
        .map(CatalogModel::new)
        .collect();
    assert_eq!(closest_model_ids("qwen/qwen3", &catalog, 5)[0], "qwen/qwen3-235b-a22b");
    assert!(closest_model_ids("gpt-oss", &catalog, 5).contains(&"openai/gpt-oss-120b".to_string()));
    assert!(closest_model_ids("zzz", &catalog, 5).is_empty());
}

#[test]
fn the_catalog_is_filtered_and_sorted_as_javascript_sorts_it() {
    let body = value(
        r#"{"data":[
            {"id":"qwen/qwen-2.5","reasoning":true,"name":"Qwen 2.5"},
            {"id":"qwen/qwen-2:free","reasoning":true,"name":"qwen/qwen-2:free"},
            {"id":"a/embed","type":"embedding","reasoning":true},
            {"id":"","reasoning":true},
            ["not","a","model"],
            {"id":"B/upper","reasoning":true,"name":""},
            {"id":"b/lower","reasoning":true}
        ]}"#,
    );
    let models = reasoning_models_from_catalog(&body);
    let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, ["b/lower", "B/upper", "qwen/qwen-2:free", "qwen/qwen-2.5"]);
    assert_eq!(models[3].name.as_deref(), Some("Qwen 2.5"));
    assert_eq!(models[1].name, None);
    assert_eq!(
        json::to_string(&serde_json::to_value(&models[0]).unwrap()),
        r#"{"id":"b/lower"}"#,
        "a model without a name has no name field"
    );
    assert_eq!(reasoning_models_from_catalog(&value(r#"[{"id":"x/y","reasoning":true}]"#)).len(), 1);
    assert!(reasoning_models_from_catalog(&value(r#"{"models":[]}"#)).is_empty());
}

#[test]
fn lists_models_over_http_with_the_key_and_openrouter_headers() {
    let server = Server::start(vec![Reply::json(
        r#"{"data":[{"id":"openai/gpt-oss-120b","supported_parameters":["reasoning"]},{"id":"openai/gpt-4o"}]}"#,
    )]);
    let client = GatewayClient::new(Gateway::OpenRouter, "sk-or-fake", server.base_url.clone());
    let models = client.list_reasoning_models().unwrap();
    assert_eq!(models, [CatalogModel::new("openai/gpt-oss-120b")]);
    let received = server.finish();
    assert_eq!(received.len(), 1);
    assert_eq!((received[0].method.as_str(), received[0].path.as_str()), ("GET", "/v1/models"));
    assert_eq!(received[0].headers["authorization"], "Bearer sk-or-fake");
    assert_eq!(received[0].headers["http-referer"], "https://athleticstandard.ai");
    assert_eq!(received[0].headers["x-title"], "Athletic Standard");
    assert!(!format!("{client:?}").contains("sk-or-fake"), "debug output hides the key");
}

#[test]
fn completes_with_the_2_1_0_request_body() {
    let server = Server::start(vec![Reply::json(
        r#"{"model":"vendor/model-2025","choices":[{"message":{"content":"{\"predicted_score\":{\"duration_s\":300}}"}}]}"#,
    )]);
    let client = GatewayClient::new(Gateway::Vercel, "vk-fake", server.base_url.clone());
    let reply = client.complete(&request("vendor/model")).unwrap();
    assert_eq!(
        reply,
        ModelReply {
            content: "{\"predicted_score\":{\"duration_s\":300}}".to_string(),
            model: "vendor/model-2025".to_string()
        }
    );
    let received = server.finish();
    assert_eq!((received[0].method.as_str(), received[0].path.as_str()), ("POST", "/v1/chat/completions"));
    assert_eq!(received[0].headers["authorization"], "Bearer vk-fake");
    assert_eq!(received[0].headers["content-type"], "application/json");
    assert!(!received[0].headers.contains_key("x-title"), "only OpenRouter gets the title headers");
    assert_eq!(
        received[0].body_text(),
        r#"{"model":"vendor/model","temperature":0,"max_tokens":8192,"messages":[{"role":"system","content":"sys"},{"role":"user","content":"{\"a\":1}"}]}"#
    );
}

#[test]
fn a_response_shape_adds_a_json_schema_format() {
    let shape = value(r#"{"type":"object"}"#);
    let body = chat_request_body(&PromptRequest { response_shape: Some(&shape), ..request("m") });
    assert_eq!(
        json::to_string(&body["response_format"]),
        r#"{"type":"json_schema","json_schema":{"name":"response","schema":{"type":"object"}}}"#
    );
}

#[test]
fn a_reply_without_text_or_model_is_handled() {
    let server = Server::start(vec![
        Reply::json(r#"{"choices":[{"message":{"content":"  "}}]}"#),
        Reply::json(r#"{"choices":[{"text":" plain "}]}"#),
        Reply::json("not json"),
    ]);
    let client = GatewayClient::new(Gateway::Vercel, "k", server.base_url.clone());
    let empty = client.complete(&request("m/one")).unwrap_err();
    assert_eq!(
        empty.to_string(),
        "the vercel gateway returned no text for m/one. Try another model from `ath models`."
    );
    let text = client.complete(&request("m/two")).unwrap();
    assert_eq!((text.content.as_str(), text.model.as_str()), ("plain", "m/two"));
    assert!(matches!(client.complete(&request("m/three")).unwrap_err(), ModelError::NoText { .. }));
    server.finish();
}

#[test]
fn gateway_refusals_name_the_status_and_remedy() {
    let server = Server::start(vec![
        Reply::status(401, "Unauthorized", r#"{"error":{"message":"Invalid API key"}}"#),
        Reply::status(429, "Too Many Requests", r#"{"error":"slow down"}"#),
        Reply::status(502, "Bad Gateway", "<html>bad</html>"),
        Reply::status(403, "Forbidden", r#"{"error":""}"#),
    ]);
    let client = GatewayClient::new(Gateway::OpenRouter, "k", server.base_url.clone());
    let messages: Vec<String> = (0..4).map(|_| client.list_reasoning_models().unwrap_err().to_string()).collect();
    assert_eq!(
        messages,
        [
            "the openrouter gateway refused the call (401). Invalid API key Check the key with `ath key`.",
            "the openrouter gateway refused the call (429). slow down",
            "the openrouter gateway refused the call (502). Bad Gateway",
            "the openrouter gateway refused the call (403). Check the key with `ath key`.",
        ]
    );
    server.finish();
}

#[test]
fn a_network_failure_is_a_transport_error() {
    let client = GatewayClient::new(Gateway::Vercel, "k", closed_port_url());
    assert!(matches!(client.list_reasoning_models().unwrap_err(), ModelError::Transport(_)));
}

#[test]
fn connecting_needs_a_key_and_prefers_the_environment() {
    let store = MemorySecretStore::new();
    let none: [(&str, &str); 0] = [];
    let endpoints = GatewayEndpoints { vercel: "http://vercel.test/v1".into(), openrouter: "http://or.test/v1".into() };

    let missing = GatewayClient::connect(&Keyring::new(&store, &none), None, &endpoints).unwrap_err();
    assert_eq!(missing, ModelError::NoKey);
    assert_eq!(missing.to_string(), no_key_message());
    assert!(missing.to_string().starts_with("A prediction needs a model, and this terminal has no gateway key."));

    store.set("openrouter", "saved-or").unwrap();
    let env = [("AI_GATEWAY_API_KEY", " env-vercel ")];
    let keyring = Keyring::new(&store, &env);
    let refused = GatewayClient::connect(&keyring, None, &endpoints).unwrap_err();
    assert!(refused.to_string().contains("Pass --gateway vercel or --gateway openrouter."));

    let vercel = GatewayClient::connect(&keyring, Some(Gateway::Vercel), &endpoints).unwrap();
    assert_eq!(vercel.gateway(), Gateway::Vercel);
    assert!(format!("{vercel:?}").contains("http://vercel.test/v1"));
    let saved_only = GatewayClient::connect(&Keyring::new(&store, &none), None, &endpoints).unwrap();
    assert_eq!(saved_only.gateway(), Gateway::OpenRouter);
    assert_eq!(
        GatewayClient::connect(&Keyring::new(&store, &none), Some(Gateway::Vercel), &endpoints).unwrap_err(),
        ModelError::NoKey
    );
}

#[test]
fn a_closure_is_a_model_client() {
    let fake = |request: &PromptRequest<'_>| -> Result<ModelReply, ModelError> {
        Ok(ModelReply { content: format!("echo {}", request.user), model: request.model.to_string() })
    };
    let client: &dyn ModelClient = &fake;
    assert_eq!(client.complete(&request("m")).unwrap().content, "echo {\"a\":1}");
}

#[test]
fn model_errors_become_library_refusals() {
    let error: athleticstandard::Error = ModelError::NoModel.into();
    assert_eq!(error.code(), "refused");
    assert!(error.to_string().starts_with("No model chosen. Pick one for this run:"));
    let transport: athleticstandard::Error = ModelError::Transport("connection refused".into()).into();
    assert_eq!((transport.code(), transport.to_string().as_str()), ("error", "connection refused"));
}
