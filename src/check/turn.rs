//! Form and completeness checks on the advocate's and the adversary's turns.

use std::collections::{BTreeMap, BTreeSet};

use super::compare::check_revision;
use super::quotes::{QuoteIndex, missing_quote_message};
use super::tree::check_tree;
use crate::config::CheckConfig;
use crate::model::{
    AdversaryTurn, AdvocateTurn, Answer, AnswerKind, CheckedTree, Claim, Critique, CritiqueId, Ground, InferenceId,
    ResponseAction, Target, Tree,
};

pub fn check_advocate_turn(
    index: &QuoteIndex,
    rules: &CheckConfig,
    turn: AdvocateTurn,
    round: u32,
    expected: &BTreeMap<CritiqueId, Critique>,
    previous: Option<&CheckedTree>,
) -> Result<(AdvocateTurn, CheckedTree), Vec<String>> {
    let mut errors = Vec::new();

    if round == 1 {
        if !turn.answers.is_empty() {
            errors.push("the first turn has no critiques to answer, so answers must be empty".into());
        }
        for claim in turn.tree.claims.iter().filter(|c| c.origin.is_some()) {
            errors.push(format!("claim {} has an origin, but the first tree adds nothing", claim.id));
        }
        for inference in turn.tree.inferences.iter().filter(|i| i.origin.is_some()) {
            errors.push(format!("inference {} has an origin, but the first tree adds nothing", inference.id));
        }
    } else {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for answer in &turn.answers {
            *seen.entry(answer.critique.as_str()).or_default() += 1;
        }
        for id in expected.keys() {
            match seen.get(id.as_str()) {
                None => errors.push(format!("critique {id} has no answer")),
                Some(n) if *n > 1 => errors.push(format!("critique {id} is answered {n} times")),
                Some(_) => {}
            }
        }
        for id in seen.keys().filter(|id| !expected.contains_key(**id)) {
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
        if let Some(critique) = expected.get(&answer.critique) {
            check_answer_kind(&owner, answer, critique, previous, &mut errors);
        }
    }
    if let Some(previous) = previous {
        let answered: Vec<(&Critique, &Answer)> =
            turn.answers.iter().filter_map(|a| expected.get(&a.critique).map(|c| (c, a))).collect();
        errors.extend(check_revision(&previous.tree, &turn.tree, round, &answered));
    }

    match check_tree(index, rules, turn.tree.clone()) {
        Ok(checked) => {
            for answer in &turn.answers {
                if let Some(critique) = expected.get(&answer.critique) {
                    check_rejection_premises(index, answer, critique, &checked, &mut errors);
                }
            }
            if errors.is_empty() { Ok((turn, checked)) } else { Err(errors) }
        }
        Err(tree_errors) => {
            errors.extend(tree_errors);
            Err(errors)
        }
    }
}

/// Which answers a critique's kind admits.
fn check_answer_kind(owner: &str, answer: &Answer, critique: &Critique, previous: Option<&CheckedTree>, errors: &mut Vec<String>) {
    match (&critique.target, answer.kind) {
        (Target::Superfluous { .. } | Target::Reading { .. }, AnswerKind::Concede) => errors.push(format!(
            "{owner}: only a gap or circular critique can be conceded; accept, adapt or reject a {} critique",
            critique.target.kind()
        )),
        (Target::Circular { premise, .. }, AnswerKind::Accept) if previous.is_some_and(|p| is_stated(&p.tree, premise)) => {
            errors.push(format!(
                "{owner}: {premise} is a premise the proposal states, and a circular critique of a stated premise cannot be accepted; concede that the inference assumes its conclusion, or reject the critique"
            ))
        }
        (Target::Gap { .. }, AnswerKind::Reject) if answer.premises.is_empty() => {
            if let Some(ground @ (Ground::Stated | Ground::Follows)) = answer.ground {
                errors.push(format!(
                    "{owner} rejects on {} but names no claims in premises; name the claims that supply the missing premise",
                    ground_name(ground)
                ));
            }
        }
        _ => {}
    }
}

/// A rejection on stated or follows claims the proposal supplies the missing premise, so the
/// claims it names must be stated premises of the inference in the revised tree.
fn check_rejection_premises(index: &QuoteIndex, answer: &Answer, critique: &Critique, revised: &CheckedTree, errors: &mut Vec<String>) {
    let (AnswerKind::Reject, Some(ground @ (Ground::Stated | Ground::Follows)), Target::Gap { inference }) =
        (answer.kind, answer.ground, &critique.target)
    else {
        return;
    };
    let owner = format!("answer to {}", answer.critique);
    let premises: &[String] = revised
        .tree
        .inferences
        .iter()
        .find(|i| &i.id == inference)
        .map(|i| i.premises.as_slice())
        .unwrap_or(&[]);
    for id in &answer.premises {
        match revised.tree.claims.iter().find(|c| &c.id == id) {
            None => errors.push(format!("{owner}: {id} is not a claim of the revised tree")),
            Some(claim) if claim.is_added() => errors.push(format!(
                "{owner}: {id} is not a stated claim; a rejection on {} names claims the proposal states",
                ground_name(ground)
            )),
            Some(_) if !premises.contains(id) => errors.push(format!(
                "{owner}: {id} is not a premise of {inference} in the revised tree; add it to the inference's premises"
            )),
            Some(_) => {}
        }
    }
    if ground == Ground::Stated {
        let named: Vec<_> = answer.premises.iter().filter_map(|id| revised.claim_spans.get(id)).flatten().collect();
        for quote in answer.quotes.iter().filter(|q| index.contains(q)) {
            let inside = index.find_all(quote).iter().any(|q| named.iter().any(|s| s.start <= q.start && q.end <= s.end));
            if !inside {
                errors.push(format!("{owner}: the quote \"{quote}\" lies within no place of a named claim"));
            }
        }
    }
}

fn is_stated(tree: &Tree, id: &str) -> bool {
    tree.claims.iter().any(|c| c.id == id && !c.is_added())
}

fn ground_name(ground: Ground) -> &'static str {
    match ground {
        Ground::Stated => "stated",
        Ground::Follows => "follows",
        Ground::CounterFails => "counter_fails",
    }
}

pub fn check_adversary_turn(
    index: &QuoteIndex,
    turn: &AdversaryTurn,
    tree: &CheckedTree,
    rejected: &BTreeSet<CritiqueId>,
    used_ids: &BTreeSet<CritiqueId>,
    conceded: &BTreeMap<CritiqueId, InferenceId>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let claims: BTreeMap<&str, &Claim> = tree.tree.claims.iter().map(|c| (c.id.as_str(), c)).collect();

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
            Target::Reading { of } => {
                if !claims.contains_key(of.as_str()) && inference(of).is_none() {
                    errors.push(format!("{owner} targets {of}, which is neither a claim nor an inference"));
                }
                if critique.quotes.is_empty() {
                    errors.push(format!("{owner} is a reading critique but quotes no place; quote the place it reads"));
                }
            }
            Target::Gap { inference: id } | Target::Superfluous { inference: id, .. } | Target::Circular { inference: id, .. } => {
                match inference(id) {
                    None => errors.push(format!("{owner} targets {id}, which is not an inference of the tree")),
                    Some(inf) => {
                        if inf.reported {
                            errors.push(format!("{owner} targets {id}, a reported inference, which takes only reading critiques"));
                        }
                        if let Some((by, _)) = conceded.iter().find(|(_, i)| *i == id) {
                            errors.push(format!(
                                "{owner} targets {id}, which the advocate conceded to fail ({by}), so it takes only reading critiques"
                            ));
                        }
                        if let Some(premise) = critique.target.premise() {
                            check_named_premise(&owner, &critique.target, premise, &inf.premises, &claims, &mut errors);
                        }
                    }
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
        if !used_ids.contains(&response.critique) {
            errors.push(format!("the response to {} names no earlier critique", response.critique));
        }
        if conceded.contains_key(&response.critique) {
            errors.push(format!("{} was conceded, so it takes no response", response.critique));
        }
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

/// A superfluous or circular critique names a premise of its inference, and a superfluous one
/// must leave the inference a premise the proposal states.
fn check_named_premise(
    owner: &str,
    target: &Target,
    premise: &str,
    premises: &[String],
    claims: &BTreeMap<&str, &Claim>,
    errors: &mut Vec<String>,
) {
    let Some(id) = target.inference() else { return };
    if !premises.iter().any(|p| p == premise) {
        errors.push(format!("{owner}: {premise} is not a premise of {id}"));
        return;
    }
    if let Target::Superfluous { .. } = target {
        if premises.len() < 2 {
            errors.push(format!("{owner}: {id} has a single premise, so it cannot lose {premise}"));
        } else if premises.iter().filter(|p| *p != premise).all(|p| claims.get(p.as_str()).is_some_and(|c| c.is_added())) {
            errors.push(format!(
                "{owner}: removing {premise} from {id} would leave only premises the advocate added; an inference keeps a premise the proposal states, so if an added premise restates the conclusion, give a circular critique on it"
            ));
        }
    }
}
