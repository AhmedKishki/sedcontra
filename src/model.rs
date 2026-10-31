//! The data the advocate, the adversary and the orchestrator exchange.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub type ClaimId = String;
pub type InferenceId = String;
pub type CritiqueId = String;

/// Where a claim or inference added during the debate came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Origin {
    pub round: u32,
    pub critique: CritiqueId,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claim {
    pub id: ClaimId,
    pub text: String,
    #[serde(default)]
    pub places: Vec<String>,
    #[serde(default)]
    pub origin: Option<Origin>,
    #[serde(default)]
    pub backed_by_context: bool,
}

impl Claim {
    /// Whether the advocate added the claim during the debate; a claim without an origin is stated.
    pub fn is_added(&self) -> bool {
        self.origin.is_some()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inference {
    pub id: InferenceId,
    #[serde(default)]
    pub premises: Vec<ClaimId>,
    pub conclusion: ClaimId,
    #[serde(default)]
    pub places: Vec<String>,
    #[serde(default)]
    pub origin: Option<Origin>,
    /// The proposal sets the inference out without drawing it in its own voice.
    #[serde(default)]
    pub reported: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Tree {
    #[serde(default)]
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub inferences: Vec<Inference>,
    #[serde(default)]
    pub conclusions: Vec<ClaimId>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnswerKind {
    Accept,
    Adapt,
    Concede,
    Reject,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ground {
    Stated,
    Follows,
    CounterFails,
}

/// The advocate's answer to one counter-proposal.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Answer {
    pub critique: CritiqueId,
    pub kind: AnswerKind,
    #[serde(default)]
    pub ground: Option<Ground>,
    #[serde(default)]
    pub reasons: String,
    #[serde(default)]
    pub quotes: Vec<String>,
    /// For a rejection on stated or follows: the claims that supply the missing premise.
    #[serde(default)]
    pub premises: Vec<ClaimId>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AdvocateTurn {
    #[serde(default)]
    pub answers: Vec<Answer>,
    pub tree: Tree,
}

/// What a counter-proposal attacks, by kind of critique.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Gap { inference: InferenceId },
    Superfluous { inference: InferenceId, premise: ClaimId },
    Circular { inference: InferenceId, premise: ClaimId },
    /// `of` names a claim or an inference.
    Reading { of: String },
}

impl Target {
    pub fn kind(&self) -> &'static str {
        match self {
            Target::Gap { .. } => "gap",
            Target::Superfluous { .. } => "superfluous",
            Target::Circular { .. } => "circular",
            Target::Reading { .. } => "reading",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Target::Gap { inference } => inference.clone(),
            Target::Superfluous { inference, premise } | Target::Circular { inference, premise } => {
                format!("{premise} in {inference}")
            }
            Target::Reading { of } => of.clone(),
        }
    }

    /// The inference a gap, superfluous or circular critique attacks.
    pub fn inference(&self) -> Option<&str> {
        match self {
            Target::Gap { inference } | Target::Superfluous { inference, .. } | Target::Circular { inference, .. } => {
                Some(inference)
            }
            Target::Reading { .. } => None,
        }
    }

    /// The premise a superfluous or circular critique names.
    pub fn premise(&self) -> Option<&str> {
        match self {
            Target::Superfluous { premise, .. } | Target::Circular { premise, .. } => Some(premise),
            Target::Gap { .. } | Target::Reading { .. } => None,
        }
    }
}

/// A counter-proposal by the adversary.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Critique {
    pub id: CritiqueId,
    #[serde(flatten)]
    pub target: Target,
    #[serde(default)]
    pub point: String,
    #[serde(default)]
    pub support: String,
    #[serde(default)]
    pub quotes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseAction {
    Drop,
    Maintain,
}

/// The adversary's response to the advocate's answer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Response {
    pub critique: CritiqueId,
    pub action: ResponseAction,
    #[serde(default)]
    pub reasons: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
    Advocate { round: u32, turn: AdvocateTurn },
    Adversary { round: u32, turn: AdversaryTurn },
}

impl Turn {
    pub fn round(&self) -> u32 {
        match self {
            Turn::Advocate { round, .. } | Turn::Adversary { round, .. } => *round,
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            Turn::Advocate { .. } => "advocate",
            Turn::Adversary { .. } => "adversary",
        }
    }
}

/// Byte offsets into the original proposal.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// A tree that passed the checks, with every place located in the proposal.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckedTree {
    pub tree: Tree,
    pub claim_spans: BTreeMap<ClaimId, Vec<Span>>,
    pub inference_spans: BTreeMap<InferenceId, Vec<Span>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ending {
    Satisfied { round: u32 },
    LimitReached,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueStatus {
    Accepted,
    Adapted,
    Conceded,
    DroppedAfterRejection,
    Open,
    Unanswered,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LedgerRow {
    pub critique: Critique,
    pub raised_in: u32,
    pub status: CritiqueStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Change {
    pub round: u32,
    pub critique: Critique,
    pub answer: Answer,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenCritique {
    pub critique: Critique,
    pub answer: Answer,
    pub response: Response,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DebateOutcome {
    pub turns: Vec<Turn>,
    pub final_tree: CheckedTree,
    pub ending: Ending,
    pub rounds_run: u32,
    pub round_limit: u32,
    pub ledger: Vec<LedgerRow>,
    pub changes: Vec<Change>,
    pub open: Vec<OpenCritique>,
    pub unanswered: Vec<Critique>,
}
