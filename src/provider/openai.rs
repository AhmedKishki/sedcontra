use std::time::Duration;

use serde_json::{Value, json};

use super::{ChatRequest, Completion, ModelProvider, ProviderError};
use crate::config::ProviderConfig;
use crate::secret::{ApiKey, load_key};

/// Attempts per call when the provider is overloaded (429, 5xx) or the connection fails;
/// the waits between them are 2 s and 4 s.
const HTTP_ATTEMPTS: u32 = 3;

/// A client for OpenAI-format chat completion APIs, such as DeepSeek's.
pub struct OpenAiCompatible {
    client: reqwest::Client,
    base_url: String,
    key: ApiKey,
}

impl OpenAiCompatible {
    pub fn from_config(provider: &str, cfg: &ProviderConfig) -> anyhow::Result<Self> {
        let key = load_key(provider, cfg)?;
        let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).build()?;
        Ok(OpenAiCompatible { client, base_url: cfg.base_url.trim_end_matches('/').to_string(), key })
    }

    /// Every error text passes through here, so a provider that echoes the key cannot pass it on.
    fn http_error(&self, error: reqwest::Error) -> ProviderError {
        ProviderError::Http(self.key.redact(&error.to_string()))
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

        let mut attempt = 1;
        let text = loop {
            let result = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(self.key.expose())
                .json(&body)
                .send()
                .await;
            let error = match result {
                Ok(response) => {
                    let status = response.status();
                    let text = response.text().await.map_err(|e| self.http_error(e))?;
                    if status.is_success() {
                        break text;
                    }
                    let error = ProviderError::Status { status: status.as_u16(), body: self.key.redact(&text) };
                    if !(status.as_u16() == 429 || status.is_server_error()) {
                        return Err(error);
                    }
                    error
                }
                Err(e) if e.is_connect() || e.is_request() => self.http_error(e),
                Err(e) => return Err(self.http_error(e)),
            };
            if attempt == HTTP_ATTEMPTS {
                return Err(error);
            }
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            attempt += 1;
        };

        let value: Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Other(format!("unreadable provider response: {e}")))?;
        let choice = &value["choices"][0];
        Ok(Completion {
            content: choice["message"]["content"].as_str().unwrap_or_default().to_string(),
            finish_reason: choice["finish_reason"].as_str().map(str::to_string),
        })
    }
}
