//! How the advocate and the adversary reach a model.

mod openai;
mod scripted;

use serde::{Deserialize, Serialize};

pub use openai::OpenAiCompatible;
pub use scripted::ScriptedProvider;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        ChatMessage { role: role.into(), content: content.into() }
    }
}

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    /// Merged over the request body, e.g. `thinking`.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct Completion {
    pub content: String,
    pub finish_reason: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ProviderError {
    #[error("request failed: {0}")]
    Http(String),
    #[error("provider returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("{0}")]
    Other(String),
}

#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, req: &ChatRequest) -> Result<Completion, ProviderError>;
}
