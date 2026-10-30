use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use super::{ChatRequest, Completion, ModelProvider, ProviderError};

/// Replays queued replies per model, for tests without a network.
#[derive(Default)]
pub struct ScriptedProvider {
    scripts: Mutex<BTreeMap<String, VecDeque<String>>>,
    calls: Mutex<Vec<String>>,
}

impl ScriptedProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, model: &str, reply: impl Into<String>) {
        self.scripts.lock().unwrap().entry(model.to_string()).or_default().push_back(reply.into());
    }

    /// The models called, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ModelProvider for ScriptedProvider {
    async fn complete(&self, req: &ChatRequest) -> Result<Completion, ProviderError> {
        self.calls.lock().unwrap().push(req.model.clone());
        let reply = self.scripts.lock().unwrap().get_mut(&req.model).and_then(VecDeque::pop_front);
        match reply {
            Some(content) => Ok(Completion { content, finish_reason: Some("stop".into()) }),
            None => Err(ProviderError::Other(format!("script exhausted for model {}", req.model))),
        }
    }
}
