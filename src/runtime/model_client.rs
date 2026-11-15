//! The narrow interface prediction and backtest use to call a model.
//!
//! Prediction rules depend on this trait, not on HTTP. The gateway client in
//! [`super::gateway`] implements it. Tests pass a closure.

use crate::error::Error;
use crate::json::Value;

use super::keyring::{Gateway, KeyRefusal};

/// One prompt for one model.
#[derive(Debug, Clone, Copy)]
pub struct PromptRequest<'a> {
    /// The model ID from the live list.
    pub model: &'a str,
    pub system: &'a str,
    pub user: &'a str,
    /// An optional JSON Schema the reply must follow. Version 2.1.0 sent
    /// none; a request without one is byte-identical to 2.1.0's.
    pub response_shape: Option<&'a Value>,
}

/// The model's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReply {
    /// The reply text. It is never empty.
    pub content: String,
    /// The model ID the gateway reports, or the requested ID when it reports
    /// none. A prediction records this ID as its author model.
    pub model: String,
}

/// Calls one model with one prompt.
pub trait ModelClient {
    fn complete(&self, request: &PromptRequest<'_>) -> Result<ModelReply, ModelError>;
}

impl<F> ModelClient for F
where
    F: Fn(&PromptRequest<'_>) -> Result<ModelReply, ModelError>,
{
    fn complete(&self, request: &PromptRequest<'_>) -> Result<ModelReply, ModelError> {
        self(request)
    }
}

/// Why a model could not be listed, chosen, or called. Each message is the
/// text 2.1.0 printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// No gateway key in the environment or the password store.
    NoKey,
    /// The password store refused, or both gateways have keys and none was named.
    Key(KeyRefusal),
    /// No `--model` and no saved default.
    NoModel,
    /// The model is not on the live list.
    UnknownModel { id: String, closest: Vec<String> },
    /// The gateway answered with an HTTP error.
    GatewayRefused { gateway: Gateway, status: u16, message: String },
    /// The gateway answered without reply text.
    NoText { gateway: Gateway, model: String },
    /// The request did not complete, or the reply could not be read.
    Transport(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::NoKey => f.write_str(&no_key_message()),
            ModelError::Key(refusal) => f.write_str(&refusal.message),
            ModelError::NoModel => f.write_str(&no_model_message()),
            ModelError::UnknownModel { id, closest } => {
                write!(f, "no model called '{id}' on the live list of text models that can reason.")?;
                if !closest.is_empty() {
                    write!(f, " Closest: {}.", closest.join(", "))?;
                }
                f.write_str(" See them all with `ath models`.")
            }
            ModelError::GatewayRefused { message, .. } => f.write_str(message),
            ModelError::NoText { gateway, model } => {
                write!(f, "the {gateway} gateway returned no text for {model}. Try another model from `ath models`.")
            }
            ModelError::Transport(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ModelError {}

impl From<KeyRefusal> for ModelError {
    fn from(refusal: KeyRefusal) -> Self {
        ModelError::Key(refusal)
    }
}

impl From<ModelError> for Error {
    fn from(error: ModelError) -> Self {
        match error {
            ModelError::Transport(message) => Error::Other(message),
            other => Error::Refused(other.to_string()),
        }
    }
}

/// The refusal when a bare terminal has no gateway key.
pub fn no_key_message() -> String {
    [
        "A prediction needs a model, and this terminal has no gateway key.",
        "",
        "Save one:  ath key set vercel",
        "       or  ath key set openrouter",
        "",
        "Or set AI_GATEWAY_API_KEY or OPENROUTER_API_KEY in the environment.",
        "",
        "In Claude Code, Cursor, or Codex the model is already there. Run",
        "`ath predict <benchmark> --json` for the evidence, then write the",
        "prediction with `ath log`. You do not need a key, and there is no",
        "Athletic Standard subscription.",
    ]
    .join("\n")
}

/// The refusal when no model was named and none is saved (D76, D78).
pub fn no_model_message() -> String {
    [
        "No model chosen. Pick one for this run:",
        "",
        "  ath models",
        "  ath predict <benchmark> --model <name>",
        "",
        "Or save your usual model:",
        "",
        "  ath models --default <name>",
    ]
    .join("\n")
}
