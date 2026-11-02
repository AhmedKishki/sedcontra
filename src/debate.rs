//! The orchestrator's debate loop between the adversary and the advocate.

use std::collections::BTreeSet;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::actor::AgentHandle;
use crate::check::{QuoteIndex, check_adversary_turn, check_advocate_turn};
use crate::model::{
    AdversaryTurn, AdvocateTurn, AnswerKind, CritiqueId, CritiqueStatus, DebateOutcome, Ending, Reply, ResponseAction,
    Thread, Turn,
};

/// The orchestrator's protocol: the JSON object each adversary turn replies with. It is appended
/// to the adversary's prompt at load, so a custom prompt cannot leave it out.
pub const ADVERSARY_SHAPE: &str = r#"Reply with one JSON object and nothing else. The shape:

{"responses": [{"critique": "K1", "action": "drop", "reasons": "…", "quotes": []},
               {"critique": "K2", "action": "press", "reasons": "…", "quotes": ["…"]}],
 "critiques": [{"id": "K3", "kind": "gap", "places": ["…"], "point": "what the step assumes that the proposal does not say", "support": "the case: the premises true and the conclusion false"},
               {"id": "K4", "kind": "conflict", "places": ["…", "…"], "point": "…", "support": "…"}],
 "satisfied": false}

In round 1 responses is empty. Copy every place and quote from the proposal character for character."#;

/// The same for the advocate.
pub const ADVOCATE_SHAPE: &str = r#"Reply with one JSON object and nothing else. The shape:

{"answers": [{"critique": "K1", "kind": "accept", "reasons": "what the critique shows and what the argument now lacks"},
             {"critique": "K2", "kind": "discuss", "ground": "misreads", "reasons": "…", "quotes": ["the words it misreads", "the place that answers it"]},
             {"critique": "K3", "kind": "discuss", "ground": "unclear", "reasons": "exactly what is missing from it", "quotes": []}]}

Copy every quote from the proposal character for character."#;

pub struct DebateInput {
    pub proposal: String,
    pub context: Option<String>,
    pub round_limit: u32,
}

pub struct TurnEvent {
    pub index: u32,
    pub max: u32,
    pub turn: Turn,
    pub raw: String,
}

#[derive(Debug)]
pub struct DebateError {
    pub reason: String,
    pub cancelled: bool,
    pub turns: Vec<Turn>,
}

pub async fn run_debate(
    input: &DebateInput,
    advocate: &AgentHandle,
    adversary: &AgentHandle,
    events: Option<&mpsc::UnboundedSender<TurnEvent>>,
    cancel: &CancellationToken,
) -> Result<DebateOutcome, DebateError> {
    let index = QuoteIndex::new(&input.proposal);
    let max = 2 * input.round_limit;
    let mut turns: Vec<Turn> = Vec::new();
    let mut used_ids: BTreeSet<CritiqueId> = BTreeSet::new();
    let mut last_advocate: Option<AdvocateTurn> = None;
    let mut ending = Ending::LimitReached;
    let mut rounds_run = 0;

    let record = |turn: Turn, raw: String, turns: &mut Vec<Turn>| {
        turns.push(turn.clone());
        if let Some(tx) = events {
            let _ = tx.send(TurnEvent { index: turns.len() as u32, max, turn, raw });
        }
    };

    for round in 1..=input.round_limit {
        rounds_run = round;

        let (message, discussed) = match &last_advocate {
            None => (
                format!("{}\n\nRaise every critique of the proposal you can substantiate.", proposal_section(input)),
                BTreeSet::new(),
            ),
            Some(last) => {
                let discussed = discussed_critiques(last);
                let instruction = if discussed.is_empty() {
                    "The advocate discussed no critique, so respond to none. Raise any new critique, or declare yourself satisfied.".to_string()
                } else {
                    format!(
                        "Respond to {}, the critiques the advocate discussed, and to no other: an accepted critique is settled. Then raise any new critique, or declare yourself satisfied.",
                        discussed.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                };
                (
                    format!("## The advocate's turn, round {}\n\n{}\n\n{instruction}", round - 1, pretty(last)),
                    discussed,
                )
            }
        };
        let (adversary_turn, raw) = take_turn(adversary, "adversary", message, cancel, &turns, |value| {
            let turn: AdversaryTurn = shaped(value)?;
            check_adversary_turn(&index, &turn, &discussed, &used_ids)?;
            Ok(turn)
        })
        .await?;
        used_ids.extend(adversary_turn.critiques.iter().map(|c| c.id.clone()));
        let expected = expected_answers(&adversary_turn);
        let message = if round == 1 {
            format!(
                "{}\n\n## The adversary's turn, round 1\n\n{}\n\nAnswer every critique.",
                proposal_section(input),
                pretty(&adversary_turn)
            )
        } else {
            format!(
                "## The adversary's turn, round {round}\n\n{}\n\nAnswer every critique pressed or raised.",
                pretty(&adversary_turn)
            )
        };
        let satisfied = adversary_turn.satisfied;
        record(Turn::Adversary { round, turn: adversary_turn }, raw, &mut turns);
        if satisfied {
            ending = Ending::Satisfied { round };
            break;
        }

        let (advocate_turn, raw) = take_turn(advocate, "advocate", message, cancel, &turns, |value| {
            let turn: AdvocateTurn = shaped(value)?;
            check_advocate_turn(&index, &turn, &expected)?;
            Ok(turn)
        })
        .await?;
        record(Turn::Advocate { round, turn: advocate_turn.clone() }, raw, &mut turns);
        last_advocate = Some(advocate_turn);
    }

    let threads = build_threads(&turns);
    Ok(DebateOutcome { turns, ending, rounds_run, round_limit: input.round_limit, threads })
}

fn proposal_section(input: &DebateInput) -> String {
    let context = input.context.as_deref().filter(|c| !c.trim().is_empty()).unwrap_or("None.");
    format!("## Proposal\n\n{}\n\n## Context from the caller\n\n{context}", input.proposal)
}

/// The critiques the advocate discussed, which the adversary must respond to.
fn discussed_critiques(last: &AdvocateTurn) -> BTreeSet<CritiqueId> {
    last.answers.iter().filter(|a| a.kind == AnswerKind::Discuss).map(|a| a.critique.clone()).collect()
}

/// The critiques the advocate must answer: those the adversary raised or pressed.
fn expected_answers(turn: &AdversaryTurn) -> BTreeSet<CritiqueId> {
    turn.critiques
        .iter()
        .map(|c| c.id.clone())
        .chain(turn.responses.iter().filter(|r| r.action == ResponseAction::Press).map(|r| r.critique.clone()))
        .collect()
}

fn pretty<T: serde::Serialize>(value: &T) -> String {
    format!("```json\n{}\n```", serde_json::to_string_pretty(value).unwrap_or_default())
}

fn shaped<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Vec<String>> {
    serde_json::from_value(value).map_err(|e| vec![format!("the reply does not have the required JSON shape: {e}")])
}

/// One turn with one retry: a reply that fails its checks is sent back with the failures.
async fn take_turn<T>(
    agent: &AgentHandle,
    role: &str,
    message: String,
    cancel: &CancellationToken,
    turns: &[Turn],
    check: impl Fn(Value) -> Result<T, Vec<String>>,
) -> Result<(T, String), DebateError> {
    let fail = |reason: String, cancelled: bool| DebateError { reason, cancelled, turns: turns.to_vec() };
    let mut text = message;
    for attempt in 0..2 {
        // The cancel branch is polled first, so no turn starts once the run is cancelled.
        let reply = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(fail("cancelled".into(), true)),
            reply = agent.turn(text) => reply.map_err(|e| fail(e.to_string(), false))?,
        };
        match check(reply.value) {
            Ok(value) => return Ok((value, reply.raw)),
            Err(errors) if attempt == 0 => {
                text = format!(
                    "Your reply failed these checks:\n{}\n\nReply again with the whole corrected JSON object.",
                    errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
                );
            }
            Err(errors) => {
                return Err(fail(format!("the {role}'s turn failed the checks twice: {}", errors.join("; ")), false));
            }
        }
    }
    unreachable!("the loop returns on its second attempt")
}

/// One thread per critique, in the order raised, with everything said about it. The last reply
/// decides the status, so a critique on which neither side has yielded is open.
pub fn build_threads(turns: &[Turn]) -> Vec<Thread> {
    let mut threads: Vec<Thread> = Vec::new();
    let position = |threads: &Vec<Thread>, id: &str| threads.iter().position(|t| t.critique.id == id);
    for turn in turns {
        match turn {
            Turn::Adversary { round, turn } => {
                for response in &turn.responses {
                    if let Some(i) = position(&threads, &response.critique) {
                        threads[i].replies.push(Reply::Adversary { round: *round, response: response.clone() });
                    }
                }
                for critique in &turn.critiques {
                    threads.push(Thread {
                        critique: critique.clone(),
                        raised_in: *round,
                        replies: Vec::new(),
                        status: CritiqueStatus::Open,
                    });
                }
            }
            Turn::Advocate { round, turn } => {
                for answer in &turn.answers {
                    if let Some(i) = position(&threads, &answer.critique) {
                        threads[i].replies.push(Reply::Advocate { round: *round, answer: answer.clone() });
                    }
                }
            }
        }
    }
    for thread in &mut threads {
        thread.status = match thread.replies.last() {
            Some(Reply::Advocate { answer, .. }) if answer.kind == AnswerKind::Accept => CritiqueStatus::Accepted,
            Some(Reply::Adversary { response, .. }) if response.action == ResponseAction::Drop => CritiqueStatus::Dropped,
            _ => CritiqueStatus::Open,
        };
    }
    threads
}
