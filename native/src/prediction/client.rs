//! The narrow interface prediction and backtest use to ask a model.
//!
//! The runtime module implements it with the Vercel AI Gateway and OpenRouter.
//! Tests implement it with a fake. Local record operations never construct one.

use serde::Serialize;

use crate::error::Result;

/// The gateways the runtime client supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gateway {
    Vercel,
    OpenRouter,
}

impl Gateway {
    pub const ALL: &'static [Gateway] = &[Gateway::Vercel, Gateway::OpenRouter];

    pub fn as_str(self) -> &'static str {
        match self {
            Gateway::Vercel => "vercel",
            Gateway::OpenRouter => "openrouter",
        }
    }

    pub fn parse(text: &str) -> Option<Gateway> {
        match text {
            "vercel" => Some(Gateway::Vercel),
            "openrouter" => Some(Gateway::OpenRouter),
            _ => None,
        }
    }
}

/// One chat completion request: a fixed system prompt and the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompleteRequest {
    pub gateway: Gateway,
    pub model: String,
    pub system: String,
    pub user: String,
}

/// The model's reply text and the model ID the gateway reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompleteResponse {
    pub content: String,
    pub model: String,
}

/// Ask a model for one completion.
///
/// An implementation returns an error for a network failure, a refusal, or an
/// empty reply. The error message is shown to the person or recorded in a
/// backtest report, so it must never contain a key.
pub trait ModelClient {
    fn complete(&mut self, request: &CompleteRequest) -> Result<CompleteResponse>;
}

impl<F> ModelClient for F
where
    F: FnMut(&CompleteRequest) -> Result<CompleteResponse>,
{
    fn complete(&mut self, request: &CompleteRequest) -> Result<CompleteResponse> {
        self(request)
    }
}
