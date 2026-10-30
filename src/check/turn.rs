//! Form and completeness checks on the advocate's and the adversary's turns.

use std::collections::{BTreeMap, BTreeSet};

use super::quotes::{QuoteIndex, missing_quote_message};
use super::tree::check_tree;
use crate::model::{
    AdversaryTurn, AdvocateTurn, AnswerKind, CheckedTree, CritiqueId, Ground, ResponseAction, Target,
};

pub fn check_advocate_turn(
    index: &QuoteIndex,
    turn: AdvocateTurn,
    round: u32,
    expected: &BTreeSet<CritiqueId>,
) -> Result<(AdvocateTurn, CheckedTree), Vec<String>> {
    let mut errors = Vec::new();

    if round == 1 {
        if !turn.answers.is_empty() {
            errors.push("the first turn has no critiques to answer, so answers must be empty".into());
        }
        for claim in turn.tree.claims.iter().filter(|c| c.origin.is_some()) {
            errors.push(format!("claim {} has an origin, but the first tree adds nothing", claim.id));
        }
    } else {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for answer in &turn.answers {
            *seen.entry(answer.critique.as_str()).or_default() += 1;
        }
        for id in expected {
            match seen.get(id.as_str()) {
                None => errors.push(format!("critique {id} has no answer")),
                Some(n) if *n > 1 => errors.push(format!("critique {id} is answered {n} times")),
                Some(_) => {}
            }
        }
        for id in seen.keys().filter(|id| !expected.contains(**id)) {
            errors.push(format!("{id} is not a critique you were asked to answer"));
        }
    }

    for answer in &turn.answers {
        let owner = format!("answer to {}", answer.critique);
        if answer.reasons.trim().is_empty() {
            errors.push(format!("{owner} gives no reasons"));
        }
        if answer.kind == AnswerKind::Reject {
            match answer.ground {
                None => errors.push(format!("{owner} rejects without a ground (stated, follows or counter_fails)")),
                Some(Ground::Stated) if answer.quotes.is_empty() => {
                    errors.push(format!("{owner} uses the ground stated but quotes no place"))
                }
                _ => {}
            }
        }
        for quote in &answer.quotes {
            if !index.contains(quote) {
                errors.push(missing_quote_message(&owner, quote));
            }
        }
    }

    match check_tree(index, turn.tree.clone()) {
        Ok(checked) if errors.is_empty() => Ok((turn, checked)),
        Ok(_) => Err(errors),
        Err(tree_errors) => {
            errors.extend(tree_errors);
            Err(errors)
        }
    }
}

pub fn check_adversary_turn(
    index: &QuoteIndex,
    turn: &AdversaryTurn,
    tree: &CheckedTree,
    rejected: &BTreeSet<CritiqueId>,
    used_ids: &BTreeSet<CritiqueId>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let claims: BTreeSet<&str> = tree.tree.claims.iter().map(|c| c.id.as_str()).collect();

    let mut new_ids = BTreeSet::new();
    for critique in &turn.critiques {
        let owner = format!("critique {}", critique.id);
        if critique.id.trim().is_empty() {
            errors.push("a critique has no id".into());
        } else if used_ids.contains(&critique.id) {
            errors.push(format!("{owner}: the id was already used by an earlier critique"));
        } else if !new_ids.insert(critique.id.as_str()) {
            errors.push(format!("{owner}: the id is used twice in this turn"));
        }
        if critique.point.trim().is_empty() {
            errors.push(format!("{owner} has no point"));
        }
        if critique.support.trim().is_empty() {
            errors.push(format!("{owner} has no support (counter-case, derivation or reading)"));
        }
        let inference = |id: &str| tree.tree.inferences.iter().find(|i| i.id == id);
        match &critique.target {
            Target::Gap { inference: id } => {
                if inference(id).is_none() {
                    errors.push(format!("{owner} targets {id}, which is not an inference of the tree"));
                }
            }
            Target::Superfluous { inference: id, premise } => match inference(id) {
                None => errors.push(format!("{owner} targets {id}, which is not an inference of the tree")),
                Some(inf) if !inf.premises.contains(premise) => {
                    errors.push(format!("{owner}: {premise} is not a premise of {id}"))
                }
                Some(inf) if inf.premises.len() < 2 => errors.push(format!(
                    "{owner}: {id} has a single premise, so it cannot lose {premise}"
                )),
                Some(_) => {}
            },
            Target::Reading { of } => {
                if !claims.contains(of.as_str()) && inference(of).is_none() {
                    errors.push(format!("{owner} targets {of}, which is neither a claim nor an inference"));
                }
            }
        }
        for quote in &critique.quotes {
            if !index.contains(quote) {
                errors.push(missing_quote_message(&owner, quote));
            }
        }
    }

    let responded: BTreeSet<&str> = turn.responses.iter().map(|r| r.critique.as_str()).collect();
    for id in rejected {
        if !responded.contains(id.as_str()) {
            errors.push(format!("the advocate rejected {id}, but there is no response to it"));
        }
    }
    for response in &turn.responses {
        if response.action == ResponseAction::Maintain && response.reasons.trim().is_empty() {
            errors.push(format!("the response maintaining {} gives no reasons", response.critique));
        }
    }

    let maintains = turn.responses.iter().any(|r| r.action == ResponseAction::Maintain);
    if turn.satisfied && (!turn.critiques.is_empty() || maintains) {
        errors.push("satisfied is true, but the turn raises or maintains critiques".into());
    }
    if !turn.satisfied && turn.critiques.is_empty() && !maintains {
        errors.push("satisfied is false, but the turn raises and maintains nothing".into());
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
