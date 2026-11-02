//! The data the adversary, the advocate and the orchestrator exchange.

use serde::{Deserialize, Serialize};

pub type CritiqueId = String;

/// A critique by the adversary against a step or a stretch of the proposal.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Critique {
    pub id: CritiqueId,
    /// As the discipline names it: gap, circularity, conflict, shift or unused.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub places: Vec<String>,
    #[serde(default)]
    pub point: String,
    #[serde(default)]
    pub support: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnswerKind {
    Accept,
    Discuss,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Ground {
    Misreads,
    Unclear,
}

/// The advocate's answer to one critique.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Answer {
    pub critique: CritiqueId,
    pub kind: AnswerKind,
    #[serde(default)]
    pub ground: Option<Ground>,
    #[serde(default)]
    pub reasons: String,
    #[serde(default)]
    pub quotes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AdvocateTurn {
    #[serde(default)]
    pub answers: Vec<Answer>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseAction {
    Drop,
    Press,
}

/// The adversary's response to an answer that discusses.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Response {
    pub critique: CritiqueId,
    pub action: ResponseAction,
    #[serde(default)]
    pub reasons: String,
    #[serde(default)]
    pub quotes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AdversaryTurn {
    #[serde(default)]
    pub responses: Vec<Response>,
    #[serde(default)]
    pub critiques: Vec<Critique>,
    #[serde(default)]
    pub satisfied: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Turn {
    Adversary { round: u32, turn: AdversaryTurn },
    Advocate { round: u32, turn: AdvocateTurn },
}

impl Turn {
    pub fn round(&self) -> u32 {
        match self {
            Turn::Adversary { round, .. } | Turn::Advocate { round, .. } => *round,
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            Turn::Adversary { .. } => "adversary",
            Turn::Advocate { .. } => "advocate",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ending {
    Satisfied { round: u32 },
    LimitReached,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CritiqueStatus {
    Accepted,
    Dropped,
    Open,
}

/// What one side said about a critique after it was raised.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "side", rename_all = "lowercase")]
pub enum Reply {
    Advocate { round: u32, answer: Answer },
    Adversary { round: u32, response: Response },
}

/// One critique with everything said about it, in order, and its status after the last turn.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Thread {
    pub critique: Critique,
    pub raised_in: u32,
    pub replies: Vec<Reply>,
    pub status: CritiqueStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DebateOutcome {
    pub turns: Vec<Turn>,
    pub ending: Ending,
    pub rounds_run: u32,
    pub round_limit: u32,
    pub threads: Vec<Thread>,
}
