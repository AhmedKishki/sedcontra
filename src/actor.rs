//! The advocate and the adversary as actors: each a tokio task with an inbox and its own history.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::agents::AgentDef;
use crate::provider::{ChatMessage, ChatRequest, ModelProvider, ProviderError};

/// Calls per turn when the reply is blank or not JSON: the first try and two retries.
const JSON_ATTEMPTS: u32 = 3;

/// Sent when a reply runs into max_tokens, so the agent decides with what it has instead of failing.
const DECIDE_NOW: &str = "Your reply was cut off at the token limit before it was complete. Stop deliberating and give your decision now: reply with the complete JSON object only, keeping every text field brief.";

/// Stands in for a cut-off reply that produced no visible text, so the roles keep alternating.
const CUT_OFF: &str = "(This reply was cut off at the token limit.)";

pub struct TurnReply {
    pub raw: String,
    pub value: Value,
}

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("the {agent}'s model call failed: {source}")]
    Provider { agent: String, source: ProviderError },
    #[error(
        "the {agent}'s reply was cut off at max_tokens even after it was told to decide at once; raise max_tokens in [models.{agent}] of the configuration"
    )]
    Truncated { agent: String },
    #[error("the {agent} returned no valid JSON in {attempts} attempts")]
    NoJson { agent: String, attempts: u32 },
    #[error("the {agent} stopped before answering")]
    Closed { agent: String },
}

struct AgentMsg {
    text: String,
    reply: oneshot::Sender<Result<TurnReply, AgentError>>,
}

/// Aborts the actor's task, and any model call in flight, when the last handle drops.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Clone)]
pub struct AgentHandle {
    name: String,
    tx: mpsc::Sender<AgentMsg>,
    _task: Arc<AbortOnDrop>,
}

pub fn spawn_agent(def: AgentDef, provider: Arc<dyn ModelProvider>) -> AgentHandle {
    let (tx, mut rx) = mpsc::channel::<AgentMsg>(4);
    let name = def.name.clone();
    let task = tokio::spawn(async move {
        let mut history = vec![ChatMessage::new("system", def.prompt.clone())];
        while let Some(msg) = rx.recv().await {
            history.push(ChatMessage::new("user", msg.text));
            let result = ask(&def, provider.as_ref(), &mut history).await;
            if let Ok(reply) = &result {
                history.push(ChatMessage::new("assistant", reply.raw.clone()));
            }
            let _ = msg.reply.send(result);
        }
    });
    AgentHandle { name, tx, _task: Arc::new(AbortOnDrop(task.abort_handle())) }
}

impl AgentHandle {
    pub async fn turn(&self, text: String) -> Result<TurnReply, AgentError> {
        let closed = || AgentError::Closed { agent: self.name.clone() };
        let (reply, rx) = oneshot::channel();
        self.tx.send(AgentMsg { text, reply }).await.map_err(|_| closed())?;
        rx.await.map_err(|_| closed())?
    }
}

/// One reply. A reply cut off at max_tokens stays in the history, followed by the instruction to
/// decide now; only a second cut-off fails the turn.
async fn ask(def: &AgentDef, provider: &dyn ModelProvider, history: &mut Vec<ChatMessage>) -> Result<TurnReply, AgentError> {
    let mut told_to_decide = false;
    let mut attempts = 0;
    loop {
        let request = ChatRequest {
            model: def.model.model.clone(),
            messages: history.clone(),
            max_tokens: def.model.max_tokens,
            extra: def.model.request.clone(),
        };
        let completion = provider
            .complete(&request)
            .await
            .map_err(|source| AgentError::Provider { agent: def.name.clone(), source })?;

        if completion.finish_reason.as_deref() == Some("length") {
            if told_to_decide {
                return Err(AgentError::Truncated { agent: def.name.clone() });
            }
            let cut = completion.content.trim();
            history.push(ChatMessage::new("assistant", if cut.is_empty() { CUT_OFF } else { cut }));
            history.push(ChatMessage::new("user", DECIDE_NOW));
            told_to_decide = true;
            continue;
        }

        attempts += 1;
        let content = strip_fence(completion.content.trim());
        if !content.is_empty()
            && let Ok(value) = serde_json::from_str::<Value>(content)
        {
            return Ok(TurnReply { raw: content.to_string(), value });
        }
        if attempts == JSON_ATTEMPTS {
            return Err(AgentError::NoJson { agent: def.name.clone(), attempts });
        }
    }
}

/// Removes a ```json fence some models put around JSON despite the JSON mode.
fn strip_fence(content: &str) -> &str {
    content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .and_then(|rest| rest.trim_end().strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(content)
}
