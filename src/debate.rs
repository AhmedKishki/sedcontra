//! The orchestrator's debate loop between the advocate and the adversary.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::actor::AgentHandle;
use crate::check::{QuoteIndex, check_adversary_turn, check_advocate_turn};
use crate::config::CheckConfig;
use crate::model::{
    AdversaryTurn, AdvocateTurn, Answer, AnswerKind, Change, CheckedTree, Critique, CritiqueId,
    CritiqueStatus, DebateOutcome, Ending, InferenceId, LedgerRow, OpenCritique, ResponseAction, Turn,
};

pub struct DebateInput {
    pub proposal: String,
    pub context: Option<String>,
    pub round_limit: u32,
    pub checks: CheckConfig,
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
    let mut critiques: BTreeMap<CritiqueId, Critique> = BTreeMap::new();
    let mut conceded: BTreeMap<CritiqueId, InferenceId> = BTreeMap::new();
    let mut last_adversary: Option<AdversaryTurn> = None;
    let mut final_tree: Option<CheckedTree> = None;
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

        let (message, expected) = match &last_adversary {
            None => (first_advocate_message(input), BTreeMap::new()),
            Some(last) => (
                format!(
                    "## The adversary's turn, round {}\n\n{}\n\nAnswer every critique raised or maintained above, then give the full revised tree.",
                    round - 1,
                    pretty(last)
                ),
                expected_answers(last)
                    .into_iter()
                    .filter_map(|id| critiques.get(&id).map(|c| (id, c.clone())))
                    .collect(),
            ),
        };
        let previous = final_tree.as_ref();
        let ((advocate_turn, checked), raw) = take_turn(advocate, "advocate", message, cancel, &turns, |value| {
            let turn: AdvocateTurn = shaped(value)?;
            check_advocate_turn(&index, &input.checks, turn, round, &expected, previous)
        })
        .await?;
        for answer in advocate_turn.answers.iter().filter(|a| a.kind == AnswerKind::Concede) {
            if let Some(inference) = expected.get(&answer.critique).and_then(|c| c.target.inference()) {
                conceded.insert(answer.critique.clone(), inference.to_string());
            }
        }
        let rejected: BTreeSet<CritiqueId> = advocate_turn
            .answers
            .iter()
            .filter(|a| a.kind == AnswerKind::Reject)
            .map(|a| a.critique.clone())
            .collect();
        let message = if round == 1 {
            format!(
                "{}\n\n## The advocate's turn, round 1\n\n{}\n\nCritique the tree.",
                proposal_section(input),
                pretty(&advocate_turn)
            )
        } else {
            format!(
                "## The advocate's turn, round {round}\n\n{}\n\nRespond to the advocate's rejections, then critique the revised tree.",
                pretty(&advocate_turn)
            )
        };
        record(Turn::Advocate { round, turn: advocate_turn }, raw, &mut turns);

        let tree = &checked;
        let (adversary_turn, raw) = take_turn(adversary, "adversary", message, cancel, &turns, |value| {
            let turn: AdversaryTurn = shaped(value)?;
            check_adversary_turn(&index, &turn, tree, &rejected, &used_ids, &conceded)?;
            Ok(turn)
        })
        .await?;
        final_tree = Some(checked);
        used_ids.extend(adversary_turn.critiques.iter().map(|c| c.id.clone()));
        critiques.extend(adversary_turn.critiques.iter().map(|c| (c.id.clone(), c.clone())));
        let satisfied = adversary_turn.satisfied;
        record(Turn::Adversary { round, turn: adversary_turn.clone() }, raw, &mut turns);
        if satisfied {
            ending = Ending::Satisfied { round };
            break;
        }
        last_adversary = Some(adversary_turn);
    }

    let final_tree = final_tree.expect("at least one round ran");
    Ok(outcome(turns, final_tree, ending, rounds_run, input.round_limit))
}

fn proposal_section(input: &DebateInput) -> String {
    let context = input.context.as_deref().filter(|c| !c.trim().is_empty()).unwrap_or("None.");
    format!("## Proposal\n\n{}\n\n## Context from the caller\n\n{context}", input.proposal)
}

fn first_advocate_message(input: &DebateInput) -> String {
    format!(
        "{}\n\n## What the checks require\n\nEach place quotes at most one sentence. These words are connectives, and each occurrence must lie within the place of an inference: {}. These words are qualifiers, and a claim keeps every one its places contain: {}.\n\nBuild the tree.",
        proposal_section(input),
        input.checks.connectives.join(", "),
        input.checks.qualifiers.join(", ")
    )
}

/// The critiques the advocate must answer: those the adversary raised or maintained in its last turn.
fn expected_answers(last: &AdversaryTurn) -> BTreeSet<CritiqueId> {
    last.critiques
        .iter()
        .map(|c| c.id.clone())
        .chain(
            last.responses
                .iter()
                .filter(|r| r.action == ResponseAction::Maintain)
                .map(|r| r.critique.clone()),
        )
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

fn outcome(turns: Vec<Turn>, final_tree: CheckedTree, ending: Ending, rounds_run: u32, round_limit: u32) -> DebateOutcome {
    let critiques: BTreeMap<&str, &Critique> = turns
        .iter()
        .filter_map(|t| match t {
            Turn::Adversary { turn, .. } => Some(turn.critiques.iter()),
            _ => None,
        })
        .flatten()
        .map(|c| (c.id.as_str(), c))
        .collect();

    let changes = turns
        .iter()
        .filter_map(|t| match t {
            Turn::Advocate { round, turn } => Some(turn.answers.iter().map(move |a| (*round, a))),
            _ => None,
        })
        .flatten()
        .filter(|(_, a)| matches!(a.kind, AnswerKind::Accept | AnswerKind::Adapt))
        .filter_map(|(round, answer)| {
            critiques.get(answer.critique.as_str()).map(|c| Change {
                round,
                critique: (*c).clone(),
                answer: answer.clone(),
            })
        })
        .collect();

    let (open, unanswered) = match (&ending, turns.last(), last_advocate_answers(&turns)) {
        (Ending::LimitReached, Some(Turn::Adversary { turn, .. }), answers) => {
            let open = turn
                .responses
                .iter()
                .filter(|r| r.action == ResponseAction::Maintain)
                .filter_map(|r| {
                    let critique = critiques.get(r.critique.as_str())?;
                    let answer = answers.iter().find(|a| a.critique == r.critique)?;
                    Some(OpenCritique { critique: (*critique).clone(), answer: answer.clone(), response: r.clone() })
                })
                .collect();
            (open, turn.critiques.clone())
        }
        _ => (Vec::new(), Vec::new()),
    };

    let ledger = build_ledger(&turns);
    DebateOutcome { turns, final_tree, ending, rounds_run, round_limit, ledger, changes, open, unanswered }
}

fn last_advocate_answers(turns: &[Turn]) -> Vec<Answer> {
    turns
        .iter()
        .rev()
        .find_map(|t| match t {
            Turn::Advocate { turn, .. } => Some(turn.answers.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// One row per critique, in the order raised, with its status after the last turn.
pub fn build_ledger(turns: &[Turn]) -> Vec<LedgerRow> {
    enum State {
        Settled(CritiqueStatus),
        Rejected,
    }
    let mut rows: Vec<(Critique, u32, State)> = Vec::new();
    let position = |rows: &Vec<(Critique, u32, State)>, id: &str| rows.iter().position(|(c, _, _)| c.id == id);

    for turn in turns {
        match turn {
            Turn::Advocate { turn, .. } => {
                for answer in &turn.answers {
                    if let Some(i) = position(&rows, &answer.critique) {
                        rows[i].2 = match answer.kind {
                            AnswerKind::Accept => State::Settled(CritiqueStatus::Accepted),
                            AnswerKind::Adapt => State::Settled(CritiqueStatus::Adapted),
                            AnswerKind::Concede => State::Settled(CritiqueStatus::Conceded),
                            AnswerKind::Reject => State::Rejected,
                        };
                    }
                }
            }
            Turn::Adversary { round, turn } => {
                for response in &turn.responses {
                    if let Some(i) = position(&rows, &response.critique) {
                        match (response.action, &rows[i].2) {
                            (ResponseAction::Maintain, _) => rows[i].2 = State::Settled(CritiqueStatus::Open),
                            (ResponseAction::Drop, State::Rejected) => {
                                rows[i].2 = State::Settled(CritiqueStatus::DroppedAfterRejection)
                            }
                            (ResponseAction::Drop, _) => {}
                        }
                    }
                }
                for critique in &turn.critiques {
                    rows.push((critique.clone(), *round, State::Settled(CritiqueStatus::Unanswered)));
                }
            }
        }
    }

    rows.into_iter()
        .map(|(critique, raised_in, state)| LedgerRow {
            critique,
            raised_in,
            status: match state {
                State::Settled(status) => status,
                // The adversary must respond to every rejection, so this only follows a failed run.
                State::Rejected => CritiqueStatus::Open,
            },
        })
        .collect()
}
