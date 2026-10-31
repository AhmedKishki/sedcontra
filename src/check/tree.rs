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

    for claim in &tree.claims {
        if claim.places.is_empty() && claim.origin.is_none() {
            errors.push(format!("claim {} has no place and no origin", claim.id));
        }
    }
    let claims: Vec<_> =
        tree.claims.iter().map(|c| (c.id.as_str(), format!("claim {}", c.id), c.places.as_slice())).collect();
    let claim_spans = locate_all(index, &claims, &mut errors);
    let inferences: Vec<_> =
        tree.inferences.iter().map(|i| (i.id.as_str(), format!("inference {}", i.id), i.places.as_slice())).collect();
    let inference_spans = locate_all(index, &inferences, &mut errors);
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
                "the connective \"{}\" in \"…{}…\" lies in no inference's place; record the inference it marks and quote the connective among that inference's places, with enough of the text around it to tell it from the connective's other occurrences",
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

/// Locates the places of every owner of one kind, claims or inferences. A quote one owner lists
/// k times takes k distinct occurrences. Each place prefers an occurrence no other owner's place
/// has taken, and the places with the fewest occurrences are located first, so that a recurring
/// connective or phrase is found where each owner states it rather than at its first occurrence.
fn locate_all(index: &QuoteIndex, owners: &[(&str, String, &[String])], errors: &mut Vec<String>) -> BTreeMap<String, Vec<Span>> {
    let mut pending: Vec<(usize, usize, Vec<Span>)> = Vec::new();
    for (o, (_, label, places)) in owners.iter().enumerate() {
        for (j, place) in places.iter().enumerate() {
            let occurrences = index.find_all(place);
            if occurrences.is_empty() {
                errors.push(missing_quote_message(label, place));
            } else {
                pending.push((o, j, occurrences));
            }
        }
    }
    pending.sort_by_key(|(_, _, occurrences)| occurrences.len());

    let mut chosen: Vec<Vec<Option<Span>>> = owners.iter().map(|(_, _, places)| vec![None; places.len()]).collect();
    let mut taken: Vec<Span> = Vec::new();
    for (o, j, occurrences) in pending {
        let own: Vec<Span> = chosen[o].iter().flatten().copied().collect();
        let free: Vec<Span> = occurrences.iter().copied().filter(|s| !own.contains(s)).collect();
        let Some(&first) = free.first() else {
            let (_, label, places) = &owners[o];
            errors.push(format!(
                "{label}: the quote \"{}\" occurs {} time(s) in the proposal but is listed more often",
                places[j],
                occurrences.len()
            ));
            continue;
        };
        let span = free.iter().copied().find(|s| !taken.iter().any(|t| s.start < t.end && t.start < s.end)).unwrap_or(first);
        taken.push(span);
        chosen[o][j] = Some(span);
    }
    owners.iter().zip(chosen).map(|((id, _, _), spans)| (id.to_string(), spans.into_iter().flatten().collect())).collect()
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
