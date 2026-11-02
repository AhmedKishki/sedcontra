//! Form and completeness checks on the adversary's and the advocate's turns.

use std::collections::{BTreeMap, BTreeSet};

use super::quotes::{QuoteIndex, missing_quote_message};
use crate::model::{AdversaryTurn, AdvocateTurn, AnswerKind, CritiqueId, Ground, ResponseAction};

/// `discussed` holds the critiques the advocate discussed in its last turn, so it is empty in
/// round 1, and `used_ids` holds every id raised so far.
pub fn check_adversary_turn(
    index: &QuoteIndex,
    turn: &AdversaryTurn,
    discussed: &BTreeSet<CritiqueId>,
    used_ids: &BTreeSet<CritiqueId>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

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
        if critique.kind.trim().is_empty() {
            errors.push(format!("{owner} names no kind"));
        }
        if critique.places.is_empty() {
            errors.push(format!("{owner} quotes no place; name what it targets by at least one place"));
        }
        for place in &critique.places {
            if !index.contains(place) {
                errors.push(missing_quote_message(&owner, place));
            }
        }
        if critique.point.trim().is_empty() {
            errors.push(format!("{owner} has no point"));
        }
        if critique.support.trim().is_empty() {
            errors.push(format!("{owner} has no support (the case, the quotes that show it, or the text)"));
        }
    }

    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for response in &turn.responses {
        *seen.entry(response.critique.as_str()).or_default() += 1;
    }
    for id in discussed {
        match seen.get(id.as_str()) {
            None => errors.push(format!("the advocate discussed {id}, but there is no response to it")),
            Some(n) if *n > 1 => errors.push(format!("{id} has {n} responses; give exactly one: drop or press")),
            Some(_) => {}
        }
    }
    let allowed = if discussed.is_empty() {
        "respond to none".to_string()
    } else {
        format!("respond only to {}", discussed.iter().cloned().collect::<Vec<_>>().join(", "))
    };
    for response in &turn.responses {
        let owner = format!("response to {}", response.critique);
        if !discussed.contains(&response.critique) {
            errors.push(format!(
                "the {owner} names no critique the advocate discussed; an accepted critique is settled and takes no response, so {allowed}"
            ));
        }
        if response.reasons.trim().is_empty() {
            errors.push(format!("the {owner} gives no reasons"));
        }
        for quote in &response.quotes {
            if !index.contains(quote) {
                errors.push(missing_quote_message(&owner, quote));
            }
        }
    }

    let presses = turn.responses.iter().any(|r| r.action == ResponseAction::Press);
    if turn.satisfied && (!turn.critiques.is_empty() || presses) {
        errors.push("satisfied is true, but the turn raises or presses critiques".into());
    }
    if !turn.satisfied && turn.critiques.is_empty() && !presses {
        errors.push("satisfied is false, but the turn raises and presses nothing".into());
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// `expected` holds the critiques raised or pressed in the adversary's last turn.
pub fn check_advocate_turn(index: &QuoteIndex, turn: &AdvocateTurn, expected: &BTreeSet<CritiqueId>) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

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

    for answer in &turn.answers {
        let owner = format!("answer to {}", answer.critique);
        if answer.reasons.trim().is_empty() {
            errors.push(format!("{owner} gives no reasons"));
        }
        if answer.kind == AnswerKind::Discuss {
            match answer.ground {
                None => errors.push(format!("{owner} discusses without a ground (misreads or unclear)")),
                Some(Ground::Misreads) if answer.quotes.is_empty() => {
                    errors.push(format!("{owner} uses the ground misreads but quotes nothing"))
                }
                Some(_) => {}
            }
        }
        for quote in &answer.quotes {
            if !index.contains(quote) {
                errors.push(missing_quote_message(&owner, quote));
            }
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
