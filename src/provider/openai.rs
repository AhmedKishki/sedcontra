use std::time::Duration;

use anyhow::bail;
use serde_json::{Value, json};

use super::{ChatRequest, Completion, ModelProvider, ProviderError};
use crate::config::ProviderConfig;

/// A client for OpenAI-format chat completion APIs, such as DeepSeek's.
pub struct OpenAiCompatible {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAiCompatible {
    pub fn from_config(cfg: &ProviderConfig) -> anyhow::Result<Self> {
        let api_key = std::env::var(&cfg.api_key_env).unwrap_or_default();
        if api_key.trim().is_empty() {
            bail!("set the environment variable {} to the provider's API key", cfg.api_key_env);
        }
        let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).build()?;
        Ok(OpenAiCompatible { client, base_url: cfg.base_url.trim_end_matches('/').to_string(), api_key })
    }
}

#[async_trait::async_trait]
impl ModelProvider for OpenAiCompatible {
    async fn complete(&self, req: &ChatRequest) -> Result<Completion, ProviderError> {
        let mut body = json!({
            "model": req.model,
            "messages": req.messages,
            "max_tokens": req.max_tokens,
            "response_format": {"type": "json_object"},
        });
        if let Value::Object(map) = &mut body {
            for (key, value) in &req.extra {
                map.insert(key.clone(), value.clone());
            }
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        let status = response.status();
        let text = response.text().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        if !status.is_success() {
            return Err(ProviderError::Status { status: status.as_u16(), body: text });
        }

        let value: Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Other(format!("unreadable provider response: {e}")))?;
        let choice = &value["choices"][0];
        Ok(Completion {
            content: choice["message"]["content"].as_str().unwrap_or_default().to_string(),
            finish_reason: choice["finish_reason"].as_str().map(str::to_string),
        })
    }
}
