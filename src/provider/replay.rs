//! A provider that serves the model replies of a run record instead of calling a model.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use anyhow::{Context, anyhow, bail};

use super::{ChatRequest, Completion, ModelProvider, ProviderError};
use crate::event::{CallEvent, CallOutcome, LogLine};
use crate::record::RunRecord;

/// The name of the event log in a run folder, which `RunRecord` keeps private.
const EVENT_LOG: &str = "events.jsonl";

struct Replies {
    queue: VecDeque<CallEvent>,
    served: usize,
}

/// Replays the model replies of a run record in the order they arrived, so the debate loop,
/// the checks and the report run again without a model.
pub struct ReplayProvider {
    replies: Mutex<Replies>,
}

impl ReplayProvider {
    pub fn from_record(dir: &Path) -> anyhow::Result<Self> {
        let calls: Vec<CallEvent> = if dir.join(EVENT_LOG).exists() {
            RunRecord::read_log(dir)?
                .into_iter()
                .filter_map(|line| match line {
                    LogLine::Call(call) => Some(call),
                    LogLine::Rejected(_) => None,
                })
                .collect()
        } else {
            calls_from_turns(dir)?
        };
        if calls.is_empty() {
            bail!("the record at {} holds no model reply", dir.display());
        }
        Ok(ReplayProvider { replies: Mutex::new(Replies { queue: calls.into(), served: 0 }) })
    }

    /// The recorded replies the run has not asked for.
    pub fn remaining(&self) -> usize {
        self.replies.lock().unwrap_or_else(PoisonError::into_inner).queue.len()
    }
}

/// The replies of a record written before the event log existed: one per turn file, which holds
/// only a reply that passed its checks.
fn calls_from_turns(dir: &Path) -> anyhow::Result<Vec<CallEvent>> {
    let turns = dir.join("turns");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&turns).with_context(|| format!("reading {}", turns.display()))? {
        let name = entry.with_context(|| format!("reading {}", turns.display()))?.file_name();
        let name = name.to_string_lossy().into_owned();
        if !name.ends_with(".parsed.json") {
            names.push(name);
        }
    }
    // Two digits order the names correctly because a debate has at most 20 turns.
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let agent = name
                .split('-')
                .nth(1)
                .ok_or_else(|| anyhow!("the turn file {name} in {} names no agent", turns.display()))?;
            let path = turns.join(&name);
            let content = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            Ok(CallEvent {
                agent: agent.to_string(),
                outcome: CallOutcome::Parsed,
                finish_reason: Some("stop".into()),
                usage: None,
                content,
            })
        })
        .collect()
}

#[async_trait::async_trait]
impl ModelProvider for ReplayProvider {
    async fn complete(&self, req: &ChatRequest) -> Result<Completion, ProviderError> {
        let mut replies = self.replies.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(next) = replies.queue.front() else {
            return Err(ProviderError::Other(format!(
                "the replay ran out of recorded replies at a call by the {}",
                req.agent
            )));
        };
        if next.agent != req.agent {
            return Err(ProviderError::Other(format!(
                "the replay diverged at call {}: the record holds a reply by the {}, and the run asked the {}",
                replies.served + 1,
                next.agent,
                req.agent
            )));
        }
        let call = next.clone();
        replies.queue.pop_front();
        replies.served += 1;
        Ok(Completion { content: call.content, finish_reason: call.finish_reason, usage: call.usage })
    }
}
