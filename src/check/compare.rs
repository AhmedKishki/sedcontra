//! Compares a revised tree with the previous one, so that every change traces back to a critique
//! answered in the same turn and every answer has the effect it claims.

use std::collections::BTreeSet;

use crate::model::{Answer, AnswerKind, Claim, ClaimId, Critique, Inference, InferenceId, Target, Tree};

/// The answers that may change what a critique touches.
const CHANGING: &[AnswerKind] = &[AnswerKind::Accept, AnswerKind::Adapt];

/// The answers that may remove a premise the advocate added.
const REVISING: &[AnswerKind] = &[AnswerKind::Accept, AnswerKind::Adapt, AnswerKind::Concede];

/// What changed between two trees. Removed inferences are given as they stood before; premises
/// compare as sets, and a claim changed when its text or places differ.
#[derive(Clone, Debug, Default)]
pub struct TreeDiff {
    pub removed_inferences: Vec<Inference>,
    pub added_inferences: Vec<InferenceId>,
    pub removed_premises: Vec<(InferenceId, ClaimId)>,
    pub added_premises: Vec<(InferenceId, ClaimId)>,
    pub changed_conclusions: Vec<InferenceId>,
    pub changed_reported: Vec<InferenceId>,
    pub removed_claims: Vec<ClaimId>,
    pub added_claims: Vec<ClaimId>,
    pub changed_claims: Vec<ClaimId>,
}

pub fn diff_trees(previous: &Tree, revised: &Tree) -> TreeDiff {
    let mut diff = TreeDiff::default();
    for before in &previous.inferences {
        let Some(after) = find_inference(revised, &before.id) else {
            diff.removed_inferences.push(before.clone());
            continue;
        };
        let had: BTreeSet<&ClaimId> = before.premises.iter().collect();
        let has: BTreeSet<&ClaimId> = after.premises.iter().collect();
        diff.removed_premises.extend(had.difference(&has).map(|p| (before.id.clone(), (*p).clone())));
        diff.added_premises.extend(has.difference(&had).map(|p| (before.id.clone(), (*p).clone())));
        if after.conclusion != before.conclusion {
            diff.changed_conclusions.push(before.id.clone());
        }
        if after.reported != before.reported {
            diff.changed_reported.push(before.id.clone());
        }
    }
    diff.added_inferences =
        revised.inferences.iter().filter(|i| find_inference(previous, &i.id).is_none()).map(|i| i.id.clone()).collect();
    for before in &previous.claims {
        match find_claim(revised, &before.id) {
            None => diff.removed_claims.push(before.id.clone()),
            Some(after) if after.text != before.text || after.places != before.places => {
                diff.changed_claims.push(before.id.clone())
            }
            Some(_) => {}
        }
    }
    diff.added_claims =
        revised.claims.iter().filter(|c| find_claim(previous, &c.id).is_none()).map(|c| c.id.clone()).collect();
    diff
}

/// Whether a critique touches an inference: it targets the inference, reads the inference or one
/// of its claims in either tree, or the inference is new and names the critique as its origin.
pub fn touches(critique: &Critique, inference: &str, previous: &Tree, revised: &Tree) -> bool {
    if critique.target.inference() == Some(inference) {
        return true;
    }
    if let Target::Reading { of } = &critique.target {
        let holds = |tree: &Tree| {
            find_inference(tree, inference).is_some_and(|i| &i.conclusion == of || i.premises.contains(of))
        };
        if of == inference || holds(previous) || holds(revised) {
            return true;
        }
    }
    find_inference(previous, inference).is_none()
        && find_inference(revised, inference).and_then(|i| i.origin.as_ref()).is_some_and(|o| o.critique == critique.id)
}

pub fn check_revision(previous: &Tree, revised: &Tree, round: u32, answered: &[(&Critique, &Answer)]) -> Vec<String> {
    let diff = diff_trees(previous, revised);
    let mut errors = Vec::new();
    let touched = |kinds: &[AnswerKind], id: &str| with_kind(answered, kinds).any(|c| touches(c, id, previous, revised));
    let read = |id: &str| with_kind(answered, CHANGING).any(|c| reads(c, id));
    let conceded: BTreeSet<&str> = with_kind(answered, &[AnswerKind::Concede]).filter_map(|c| c.target.inference()).collect();

    let moved = diff.removed_inferences.iter().map(|i| i.id.as_str()).chain(diff.changed_conclusions.iter().map(String::as_str));
    for id in moved {
        if conceded.contains(id) {
            errors.push(format!("inference {id} was conceded in this turn, so it stays in the tree as the proposal draws it"));
        } else if !touched(CHANGING, id) {
            errors.push(format!(
                "inference {id} leaves the tree without a critique that touches it; change an inference only in answer to a critique on it"
            ));
        }
    }
    for (inference, premise) in &diff.removed_premises {
        if is_stated(previous, premise) {
            let named = with_kind(answered, CHANGING).any(|c| {
                matches!(&c.target, Target::Superfluous { inference: i, premise: p } if i == inference && p == premise)
            });
            if !named && !read(premise) && !read(inference) {
                errors.push(format!(
                    "{premise} left {inference}, but it is a premise the proposal states; remove it only in answer to a superfluous critique that names it or a reading critique"
                ));
            }
        } else if !touched(REVISING, inference) {
            errors.push(format!("{premise} left {inference} without a critique answered in this turn that touches {inference}"));
        }
    }
    for id in diff.removed_claims.iter().filter(|id| is_stated(previous, id)) {
        if !read(id) {
            errors.push(format!(
                "claim {id} leaves the tree, but the proposal states it; keep it, since a claim no inference uses is reported as a stub"
            ));
        }
    }
    for id in &diff.changed_reported {
        if !read(id) {
            errors.push(format!("inference {id} changed its reported flag without a reading critique on it"));
        }
    }

    let answered_ids: BTreeSet<&str> = answered.iter().map(|(c, _)| c.id.as_str()).collect();
    let current = |origin: Option<&crate::model::Origin>| {
        origin.is_some_and(|o| o.round == round && answered_ids.contains(o.critique.as_str()))
    };
    for claim in diff.added_claims.iter().filter_map(|id| find_claim(revised, id)).filter(|c| c.is_added()) {
        if !current(claim.origin.as_ref()) {
            errors.push(format!("claim {} is new, so its origin must name round {round} and a critique answered in it", claim.id));
        } else if !answered.iter().filter(|(c, _)| claim.origin.as_ref().is_some_and(|o| o.critique == c.id)).any(|(c, _)| {
            revised.inferences.iter().any(|i| i.premises.contains(&claim.id) && touches(c, &i.id, previous, revised))
        }) {
            errors.push(format!("claim {} is not a premise of an inference its origin critique touches", claim.id));
        }
    }
    for id in &diff.added_inferences {
        if !current(find_inference(revised, id).and_then(|i| i.origin.as_ref())) {
            errors.push(format!("inference {id} is new, so it needs an origin naming round {round} and a critique answered in it"));
        }
    }

    for (critique, answer) in answered {
        if let Some(missing) = missing_effect(critique, answer, previous, revised, &diff) {
            errors.push(format!("answer to {}: {missing}", critique.id));
        }
    }
    errors
}

/// What an answer failed to do to the tree, if anything.
fn missing_effect(critique: &Critique, answer: &Answer, previous: &Tree, revised: &Tree, diff: &TreeDiff) -> Option<String> {
    match (answer.kind, &critique.target) {
        (AnswerKind::Accept, Target::Gap { inference }) => {
            let gained = diff.added_premises.iter().any(|(i, p)| {
                i == inference && find_claim(revised, p).and_then(|c| c.origin.as_ref()).is_some_and(|o| o.critique == critique.id)
            });
            (!gained).then(|| format!("accepting a gap gives {inference} an added premise whose origin is {}", critique.id))
        }
        (AnswerKind::Accept, Target::Superfluous { inference, premise } | Target::Circular { inference, premise }) => {
            let removed = find_inference(revised, inference).is_some_and(|i| !i.premises.contains(premise));
            (!removed).then(|| format!("accepting the critique removes {premise} from {inference}, which stays in the tree"))
        }
        (AnswerKind::Accept, Target::Reading { of }) => (!reading_target_changed(of, previous, revised, diff))
            .then(|| format!("accepting a reading critique changes {of} or an inference it belongs to")),
        (AnswerKind::Adapt, target) => {
            let target_changed = matches!(target, Target::Reading { of } if reading_target_changed(of, previous, revised, diff));
            let ids: BTreeSet<&str> = previous.inferences.iter().chain(&revised.inferences).map(|i| i.id.as_str()).collect();
            let touched_changed =
                ids.into_iter().any(|id| touches(critique, id, previous, revised) && inference_changed(id, previous, revised, diff));
            (!target_changed && !touched_changed)
                .then(|| "adapting a critique changes its target, an inference it touches, or a claim of such an inference".to_string())
        }
        (AnswerKind::Concede, Target::Gap { inference } | Target::Circular { inference, .. }) => {
            match find_inference(revised, inference) {
                None => Some(format!("a conceded inference stays in the tree, but {inference} is gone")),
                Some(kept) => match &critique.target {
                    Target::Circular { premise, .. } if !is_stated(previous, premise) && kept.premises.contains(premise) => Some(
                        format!("conceding a circular critique of {premise}, which you added, removes it from {inference}"),
                    ),
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

/// Whether an inference was removed, added or changed, counting a change to the text or places
/// of one of its claims.
fn inference_changed(id: &str, previous: &Tree, revised: &Tree, diff: &TreeDiff) -> bool {
    match (find_inference(previous, id), find_inference(revised, id)) {
        (Some(before), Some(after)) => {
            let premises = |i: &Inference| i.premises.iter().cloned().collect::<BTreeSet<_>>();
            premises(before) != premises(after)
                || before.conclusion != after.conclusion
                || before.places != after.places
                || before.reported != after.reported
                || before.premises.iter().chain([&before.conclusion]).any(|c| diff.changed_claims.contains(c))
        }
        (None, None) => false,
        _ => true,
    }
}

/// Whether the claim or inference a reading critique targets changed, or an inference holding
/// the claim did.
fn reading_target_changed(of: &str, previous: &Tree, revised: &Tree, diff: &TreeDiff) -> bool {
    if find_inference(previous, of).is_some() || find_inference(revised, of).is_some() {
        return inference_changed(of, previous, revised, diff);
    }
    diff.removed_claims.iter().chain(&diff.changed_claims).any(|c| c == of)
        || previous
            .inferences
            .iter()
            .chain(&revised.inferences)
            .filter(|i| i.conclusion == of || i.premises.iter().any(|p| p == of))
            .any(|i| inference_changed(&i.id, previous, revised, diff))
}

fn with_kind<'a>(answered: &'a [(&'a Critique, &'a Answer)], kinds: &'a [AnswerKind]) -> impl Iterator<Item = &'a Critique> + 'a {
    answered.iter().filter(move |(_, a)| kinds.contains(&a.kind)).map(|(c, _)| *c)
}

fn reads(critique: &Critique, id: &str) -> bool {
    matches!(&critique.target, Target::Reading { of } if of == id)
}

fn is_stated(tree: &Tree, id: &str) -> bool {
    find_claim(tree, id).is_some_and(|c| !c.is_added())
}

fn find_inference<'a>(tree: &'a Tree, id: &str) -> Option<&'a Inference> {
    tree.inferences.iter().find(|i| i.id == id)
}

fn find_claim<'a>(tree: &'a Tree, id: &str) -> Option<&'a Claim> {
    tree.claims.iter().find(|c| c.id == id)
}
