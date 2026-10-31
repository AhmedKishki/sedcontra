//! The structural checks every tree from the advocate must pass.

use std::collections::{BTreeMap, BTreeSet};

use super::quotes::{QuoteIndex, find_words, missing_quote_message, normalise, sentence_break, words};
use crate::config::CheckConfig;
use crate::model::{CheckedTree, ClaimId, InferenceId, Span, Tree};

/// Bytes of the proposal quoted on each side of an uncited connective, enough to find it.
const CONTEXT_BYTES: usize = 40;

pub fn check_tree(index: &QuoteIndex, rules: &CheckConfig, tree: Tree) -> Result<CheckedTree, Vec<String>> {
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
    check_places(index, rules, &tree, &claim_spans, &inference_spans, &mut errors);
    for inference in &tree.inferences {
        let added = |id: &ClaimId| tree.claims.iter().any(|c| &c.id == id && c.is_added());
        if !inference.premises.is_empty() && inference.premises.iter().all(added) {
            errors.push(format!(
                "inference {} rests only on premises the advocate added ({}); keep a premise the proposal states",
                inference.id,
                inference.premises.join(", ")
            ));
        }
    }

    if errors.is_empty() {
        Ok(CheckedTree { tree, claim_spans, inference_spans })
    } else {
        Err(errors)
    }
}

/// What each place may quote: one sentence for a claim, every connective inside an inference's
/// place, different words for a premise and its conclusion, and every qualifier kept in the claim.
fn check_places(
    index: &QuoteIndex,
    rules: &CheckConfig,
    tree: &Tree,
    claim_spans: &BTreeMap<ClaimId, Vec<Span>>,
    inference_spans: &BTreeMap<InferenceId, Vec<Span>>,
    errors: &mut Vec<String>,
) {
    let proposal = index.proposal();
    for claim in &tree.claims {
        for span in claim_spans.get(&claim.id).into_iter().flatten() {
            let quoted = &proposal[span.start..span.end];
            if let Some(end) = sentence_break(quoted) {
                errors.push(format!(
                    "claim {}: the place \"{}\" runs past a sentence end after \"{}\"; quote one sentence per place, and give each assertion its own claim",
                    claim.id,
                    normalise(quoted),
                    normalise(&quoted[..end])
                ));
            }
        }
    }

    let cited: Vec<&Span> = inference_spans.values().flatten().collect();
    for word in find_words(proposal, &rules.connectives) {
        if !cited.iter().any(|s| s.start <= word.start && word.end <= s.end) {
            errors.push(format!(
                "the connective \"{}\" in \"…{}…\" lies in no inference's place; record the inference it marks and quote the connective among that inference's places",
                &proposal[word.start..word.end],
                context(proposal, word)
            ));
        }
    }

    for inference in &tree.inferences {
        let conclusion = claim_spans.get(&inference.conclusion).map(Vec::as_slice).unwrap_or(&[]);
        for premise in &inference.premises {
            for p in claim_spans.get(premise).into_iter().flatten() {
                if let Some(c) = conclusion.iter().find(|c| p.start < c.end && c.start < p.end) {
                    errors.push(format!(
                        "inference {}: premise {premise} and conclusion {} quote the same words \"{}\"; quote the words that state each",
                        inference.id,
                        inference.conclusion,
                        normalise(&proposal[p.start.max(c.start)..p.end.min(c.end)])
                    ));
                }
            }
        }
    }

    let qualifiers: BTreeSet<String> = rules.qualifiers.iter().map(|w| w.replace('’', "'").to_lowercase()).collect();
    for claim in tree.claims.iter().filter(|c| !c.is_added()) {
        let kept = words(&claim.text);
        let dropped: BTreeSet<String> =
            claim.places.iter().flat_map(|p| words(p)).filter(|w| qualifiers.contains(w) && !kept.contains(w)).collect();
        for word in dropped {
            errors.push(format!(
                "claim {}: its place has \"{word}\", which its text drops; keep the proposal's negation, modal and quantifier words in the claim",
                claim.id
            ));
        }
    }
}

/// The proposal around `span`, cut on character boundaries.
fn context(text: &str, span: Span) -> String {
    let mut start = span.start.saturating_sub(CONTEXT_BYTES);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (span.end + CONTEXT_BYTES).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    normalise(&text[start..end])
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
