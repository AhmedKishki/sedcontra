//! What a run reports while it is under way: every model reply, every rejection and every turn.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Turn;
use crate::provider::Usage;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome {
    Parsed,
    CutOff,
    NotJson,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CallEvent {
    pub agent: String,
    pub outcome: CallOutcome,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rejection {
    pub agent: String,
    pub round: u32,
    pub failures: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LogLine {
    Call(CallEvent),
    Rejected(Rejection),
}

pub struct TurnEvent {
    pub index: u32,
    pub max: u32,
    pub turn: Turn,
    pub raw: String,
}

pub enum RunEvent {
    Log(LogLine),
    Turn(TurnEvent),
}

/// The usage of a whole run: for every `Call` line that has a usage object, and for every key of
/// that object whose value is a JSON unsigned integer, the value is added to the total under the
/// same key. Every other value is skipped, so nested detail objects do not enter the totals.
pub fn total_usage(log: &[LogLine]) -> Usage {
    let mut total = Usage::new();
    for line in log {
        let LogLine::Call(call) = line else { continue };
        let Some(usage) = &call.usage else { continue };
        for (key, value) in usage {
            let Some(count) = value.as_u64() else { continue };
            let sum = total.get(key).and_then(Value::as_u64).unwrap_or(0).saturating_add(count);
            total.insert(key.clone(), Value::from(sum));
        }
    }
    total
}
