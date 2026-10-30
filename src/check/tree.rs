//! The structural checks every tree from the advocate must pass.

use std::collections::{BTreeMap, BTreeSet};

use super::quotes::{QuoteIndex, missing_quote_message, normalise};
use crate::model::{CheckedTree, ClaimId, Span, Tree};

pub fn check_tree(index: &QuoteIndex, tree: Tree) -> Result<CheckedTree, Vec<String>> {
    let mut errors = Vec::new();

    let mut claim_ids = BTreeSet::new();
    for claim in &tree.claims {
        if !claim_ids.insert(claim.id.as_str()) {
            errors.push(format!("claim id {} is used more than once", claim.id));
        }
    }
    let mut inference_ids = BTreeSet::new();
    for inference in &tree.inferences {
        if !inference_ids.insert(inference.id.as_str()) {
            errors.push(format!("inference id {} is used more than once", inference.id));
        }
        if inference.premises.is_empty() {
            errors.push(format!("inference {} has no premises", inference.id));
        }
        for id in inference.premises.iter().chain([&inference.conclusion]) {
            if !claim_ids.contains(id.as_str()) {
                errors.push(format!("inference {} refers to unknown claim {id}", inference.id));
            }
        }
    }
    if tree.conclusions.is_empty() {
        errors.push("no conclusion is marked".into());
    }
    for id in &tree.conclusions {
        if !claim_ids.contains(id.as_str()) {
            errors.push(format!("conclusion {id} is not a claim"));
        }
    }
    if let Some(cycle) = find_loop(&tree) {
        errors.push(format!("the inferences form a loop: {}", cycle.join(" → ")));
    }

    let mut claim_spans = BTreeMap::new();
    for claim in &tree.claims {
        if claim.places.is_empty() && claim.origin.is_none() {
            errors.push(format!("claim {} has no place and no origin", claim.id));
        }
        let owner = format!("claim {}", claim.id);
        claim_spans.insert(claim.id.clone(), locate(index, &owner, &claim.places, &mut errors));
    }
    let mut inference_spans = BTreeMap::new();
    for inference in &tree.inferences {
        let owner = format!("inference {}", inference.id);
        inference_spans.insert(inference.id.clone(), locate(index, &owner, &inference.places, &mut errors));
    }

    if errors.is_empty() {
        Ok(CheckedTree { tree, claim_spans, inference_spans })
    } else {
        Err(errors)
    }
}

/// Gives each place its own occurrence: a quote listed k times takes the first k occurrences.
fn locate(index: &QuoteIndex, owner: &str, places: &[String], errors: &mut Vec<String>) -> Vec<Span> {
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let mut spans = Vec::new();
    for place in places {
        let key = normalise(place);
        let occurrences = index.find_all(place);
        let nth = used.entry(key).or_insert(0);
        match occurrences.get(*nth) {
            Some(span) => spans.push(*span),
            None if occurrences.is_empty() => errors.push(missing_quote_message(owner, place)),
            None => errors.push(format!(
                "{owner}: the quote \"{place}\" occurs {} time(s) in the proposal but is listed more often",
                occurrences.len()
            )),
        }
        *nth += 1;
    }
    spans
}

/// A loop in the edges premise → conclusion, as the path of claim ids that closes it.
fn find_loop(tree: &Tree) -> Option<Vec<ClaimId>> {
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for inference in &tree.inferences {
        for premise in &inference.premises {
            edges.entry(premise.as_str()).or_default().push(inference.conclusion.as_str());
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Colour {
        White,
        Grey,
        Black,
    }
    let mut colour: BTreeMap<&str, Colour> = BTreeMap::new();
    let mut stack: Vec<&str> = Vec::new();

    fn visit<'a>(
        node: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        colour: &mut BTreeMap<&'a str, Colour>,
        stack: &mut Vec<&'a str>,
    ) -> Option<Vec<ClaimId>> {
        colour.insert(node, Colour::Grey);
        stack.push(node);
        for &next in edges.get(node).map(Vec::as_slice).unwrap_or(&[]) {
            match colour.get(next).copied().unwrap_or(Colour::White) {
                Colour::Grey => {
                    let from = stack.iter().position(|&n| n == next).unwrap_or(0);
                    let mut path: Vec<ClaimId> = stack[from..].iter().map(|s| s.to_string()).collect();
                    path.push(next.to_string());
                    return Some(path);
                }
                Colour::White => {
                    if let Some(path) = visit(next, edges, colour, stack) {
                        return Some(path);
                    }
                }
                Colour::Black => {}
            }
        }
        stack.pop();
        colour.insert(node, Colour::Black);
        None
    }

    let nodes: Vec<&str> = edges.keys().copied().collect();
    for node in nodes {
        if colour.get(node).copied().unwrap_or(Colour::White) == Colour::White
            && let Some(path) = visit(node, &edges, &mut colour, &mut stack)
        {
            return Some(path);
        }
    }
    None
}
