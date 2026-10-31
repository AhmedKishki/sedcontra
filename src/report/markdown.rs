//! The report as markdown, for an agent or a person to read.

use std::fmt::Write;

use super::{Report, render_argdown};
use crate::analyze::Finding;
use crate::check::unsupported_conclusions;
use crate::model::{
    AdversaryTurn, AdvocateTurn, Answer, AnswerKind, Critique, CritiqueStatus, Effect, Ending, Ground, LedgerRow, Target,
    Tree, Turn,
};

pub fn render_markdown(report: &Report) -> String {
    let mut out = String::new();
    let tree = &report.final_tree.tree;
    let conceded = report.conceded_by_inference();

    let _ = writeln!(out, "# Cross-examination report\n");
    let _ = writeln!(out, "Run `{}`, record in `{}`.\n", report.run_id, report.record_dir.display());

    let _ = writeln!(out, "## Ending\n");
    let ending = match report.ending {
        Ending::Satisfied { round } => format!(
            "The adversary was satisfied in round {round} of at most {}: it could show no critique of the final tree that meets explanatory-critique.",
            report.round_limit
        ),
        Ending::LimitReached => format!(
            "The round limit of {} was reached with {} open and {} unanswered critiques. Open critiques were argued by both sides. Unanswered critiques were raised in the adversary's last turn, and the advocate had no turn to examine them.",
            report.round_limit,
            report.open.len(),
            report.unanswered.len()
        ),
    };
    let unsupported = unsupported_conclusions(tree).len();
    let counts = format!(
        "{unsupported} of {} {} no supporting inference, and {} conceded to fail.",
        tree.conclusions.len(),
        if unsupported == 1 { "conclusions has" } else { "conclusions have" },
        if conceded.len() == 1 { "1 inference was".to_string() } else { format!("{} inferences were", conceded.len()) }
    );
    let _ = writeln!(out, "{ending} {counts}\n\nThis describes how the debate ended. It is not a verdict on the proposal.\n");

    let _ = writeln!(out, "## Conclusions\n");
    for support in &report.conclusions {
        let text = tree.claims.iter().find(|c| c.id == support.conclusion).map(|c| one_line(&c.text)).unwrap_or_default();
        if !support.supported {
            let _ = writeln!(out, "- **{}** {text}: no inference supports it.", support.conclusion);
            continue;
        }
        let parts: Vec<String> = [
            ("rests on added", &support.added),
            ("passes through conceded", &support.conceded),
            ("open", &support.open),
            ("unanswered", &support.unanswered),
        ]
        .into_iter()
        .filter(|(_, ids)| !ids.is_empty())
        .map(|(label, ids)| format!("{label} {}", ids.join(", ")))
        .collect();
        let rests = if parts.is_empty() { String::new() } else { format!(" {}.", parts.join("; ")) };
        let _ = writeln!(
            out,
            "- **{}** {text}:{rests} Some line of support avoids every conceded inference: {}.",
            support.conclusion,
            if support.intact { "yes" } else { "no" }
        );
    }
    out.push('\n');

    let _ = writeln!(out, "## Critique ledger\n");
    if report.ledger.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Critique | Kind | Target | Raised in | Status |\n| --- | --- | --- | --- | --- |");
        for row in &report.ledger {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                row.critique.id,
                row.critique.target.kind(),
                row.critique.target.describe(),
                row.raised_in,
                ledger_status(row, &report.transcript)
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Final tree\n\n```argdown\n{}```\n", render_argdown(tree, &conceded));
    let _ = writeln!(out, "### Places\n");
    for claim in &tree.claims {
        let source = match &claim.origin {
            Some(origin) => format!(
                "added in round {} for {}{}",
                origin.round,
                origin.critique,
                if claim.backed_by_context { " (backed by context)" } else { "" }
            ),
            None => quotes(&claim.places),
        };
        let _ = writeln!(out, "- **{}** {}: {}", claim.id, one_line(&claim.text), source);
    }
    for inference in &tree.inferences {
        let source = match &inference.origin {
            Some(origin) => format!("added in round {} for {}", origin.round, origin.critique),
            None if inference.places.is_empty() => "no connective in the text".into(),
            None => quotes(&inference.places),
        };
        let mark = match conceded.get(&inference.id) {
            Some(by) => format!("; conceded ({})", by.join(", ")),
            None if inference.reported => "; reported".into(),
            None => String::new(),
        };
        let _ = writeln!(
            out,
            "- **{}** {} → {}: {}{}",
            inference.id,
            inference.premises.join(", "),
            inference.conclusion,
            source,
            mark
        );
    }
    out.push('\n');

    let _ = writeln!(out, "## Added claims\n");
    let added: Vec<_> = tree.claims.iter().filter_map(|c| c.origin.as_ref().map(|o| (c, o))).collect();
    if added.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for (claim, origin) in added {
        let completes: Vec<&str> =
            tree.inferences.iter().filter(|i| i.premises.contains(&claim.id)).map(|i| i.id.as_str()).collect();
        let _ = writeln!(
            out,
            "- **{}** {}: completes {}, exposed by {} in round {}",
            claim.id,
            one_line(&claim.text),
            if completes.is_empty() { "no inference".to_string() } else { completes.join(", ") },
            origin.critique,
            origin.round
        );
    }
    out.push('\n');

    let _ = writeln!(out, "## Stated premises that close a gap\n");
    if report.closed.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for change in &report.closed {
        let closing: Vec<String> = change
            .answer
            .premises
            .iter()
            .map(|id| match tree.claims.iter().find(|c| &c.id == id) {
                Some(claim) => format!("{id} {}", quotes(&claim.places)),
                None => id.clone(),
            })
            .collect();
        let _ = writeln!(
            out,
            "- **{}** ({}): closed by {}. Counter-case: {}",
            change.critique.id,
            target_phrase(&change.critique),
            closing.join("; "),
            one_line(&change.critique.support)
        );
    }
    out.push('\n');

    let _ = writeln!(out, "## Conceded inferences\n");
    if report.conceded.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for change in &report.conceded {
        let Some(id) = change.critique.target.inference() else { continue };
        let shape = tree
            .inferences
            .iter()
            .find(|i| i.id == id)
            .map(|i| format!(" ({} → {})", i.premises.join(", "), i.conclusion))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "- **{id}**{shape} fails: {} {} Support: {}",
            change.critique.id,
            sentence(&change.critique.point),
            one_line(&change.critique.support)
        );
    }
    out.push('\n');

    let _ = writeln!(out, "## Changes\n");
    if report.changes.is_empty() {
        let _ = writeln!(out, "None.\n");
    }
    for change in &report.changes {
        let _ = writeln!(
            out,
            "- **{}** ({}, {} in round {}): {} The advocate: {}{}",
            change.critique.id,
            target_phrase(&change.critique),
            kind_label(&change.answer),
            change.round,
            sentence(&change.critique.point),
            sentence(&change.answer.reasons),
            effects_phrase(&change.effects, tree)
        );
    }
    if !report.changes.is_empty() {
        out.push('\n');
    }

    let _ = writeln!(out, "## Open critiques\n");
    if report.open.is_empty() {
        let _ = writeln!(out, "None.\n");
    }
    for open in &report.open {
        let _ = writeln!(out, "### {}: {}\n", open.critique.id, target_phrase(&open.critique));
        let _ = writeln!(out, "- Critique: {}", one_line(&open.critique.point));
        let _ = writeln!(out, "- Support: {}", one_line(&open.critique.support));
        let _ = writeln!(out, "- The advocate ({}): {}", kind_label(&open.answer), one_line(&open.answer.reasons));
        let _ = writeln!(out, "- The adversary maintained it: {}\n", one_line(&open.response.reasons));
    }

    let _ = writeln!(out, "## Unanswered critiques\n");
    if report.unanswered.is_empty() {
        let _ = writeln!(out, "None.\n");
    }
    for critique in &report.unanswered {
        let _ = writeln!(
            out,
            "- **{}** ({}): {} Support: {}",
            critique.id,
            target_phrase(critique),
            sentence(&critique.point),
            one_line(&critique.support)
        );
    }
    if !report.unanswered.is_empty() {
        out.push('\n');
    }

    let _ = writeln!(out, "## Structural findings\n");
    render_findings(&mut out, &report.findings, tree);

    let _ = writeln!(out, "## Transcript\n");
    out.push_str(&render_transcript(&report.transcript));
    out
}

fn render_findings(out: &mut String, findings: &[Finding], tree: &Tree) {
    let text_of = |id: &str| tree.claims.iter().find(|c| c.id == id).map(|c| one_line(&c.text)).unwrap_or_default();

    let _ = writeln!(out, "### Restatements\n");
    let restatements: Vec<_> = findings
        .iter()
        .filter_map(|f| match f {
            Finding::Restatement { claim, quotes: q, .. } => Some((claim, q)),
            _ => None,
        })
        .collect();
    if restatements.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for (claim, q) in restatements {
        let _ = writeln!(out, "- **{claim}** is stated {} times: {}. Keep one.", q.len(), quotes(q));
    }
    out.push('\n');

    let _ = writeln!(out, "### Uncovered text\n");
    let uncovered: Vec<_> = findings
        .iter()
        .filter_map(|f| match f {
            Finding::Uncovered { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    if uncovered.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for text in uncovered {
        let _ = writeln!(out, "- \"{}\"", one_line(text));
    }
    out.push('\n');

    let _ = writeln!(out, "### Stubs\n");
    let mut stubs: Vec<_> = findings
        .iter()
        .filter_map(|f| match f {
            Finding::Stub { claims, prune } => Some((claims, *prune)),
            _ => None,
        })
        .collect();
    stubs.sort_by_key(|(_, prune)| !prune);
    if stubs.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for (claims, prune) in stubs {
        let label = if prune { "Prune suggestion" } else { "Separate line of argument" };
        let listed: Vec<String> = claims.iter().map(|id| format!("{id} ({})", text_of(id))).collect();
        let _ = writeln!(out, "- {label}: {}", listed.join("; "));
    }
    out.push('\n');

    let _ = writeln!(out, "### Unsupported conclusions\n");
    let unsupported: Vec<_> = findings
        .iter()
        .filter_map(|f| match f {
            Finding::Unsupported { claim } => Some(claim),
            _ => None,
        })
        .collect();
    if unsupported.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for id in unsupported {
        let places = tree.claims.iter().find(|c| &c.id == id).map(|c| quotes(&c.places)).unwrap_or_default();
        let _ = writeln!(out, "- **{id}** {}: {places}", text_of(id));
    }
    out.push('\n');
}

pub fn render_transcript(turns: &[Turn]) -> String {
    let mut out = String::new();
    for turn in turns {
        let _ = writeln!(out, "### Round {}: {}\n", turn.round(), turn.role());
        match turn {
            Turn::Advocate { turn, .. } => render_advocate(&mut out, turn),
            Turn::Adversary { turn, .. } => render_adversary(&mut out, turn),
        }
        out.push('\n');
    }
    out
}

fn render_advocate(out: &mut String, turn: &AdvocateTurn) {
    for answer in &turn.answers {
        let _ = writeln!(out, "- Answer to {}: {}. {}", answer.critique, kind_label(answer), one_line(&answer.reasons));
    }
    if !turn.answers.is_empty() {
        out.push('\n');
    }
    for claim in &turn.tree.claims {
        let _ = writeln!(out, "- {}: {}", claim.id, one_line(&claim.text));
    }
    for inference in &turn.tree.inferences {
        let _ = writeln!(out, "- {}: {} → {}", inference.id, inference.premises.join(", "), inference.conclusion);
    }
    let _ = writeln!(out, "- Conclusions: {}", turn.tree.conclusions.join(", "));
}

fn render_adversary(out: &mut String, turn: &AdversaryTurn) {
    for response in &turn.responses {
        let action = match response.action {
            crate::model::ResponseAction::Drop => "dropped",
            crate::model::ResponseAction::Maintain => "maintained",
        };
        let _ = writeln!(out, "- {} {action}. {}", response.critique, one_line(&response.reasons));
    }
    for critique in &turn.critiques {
        let _ = writeln!(
            out,
            "- {} ({}): {} Support: {}",
            critique.id,
            target_phrase(critique),
            sentence(&critique.point),
            one_line(&critique.support)
        );
    }
    let _ = writeln!(out, "- Satisfied: {}", if turn.satisfied { "yes" } else { "no" });
}

/// What a change did to the tree, with the text of each claim it added.
fn effects_phrase(effects: &[Effect], tree: &Tree) -> String {
    let text = |id: &str| tree.claims.iter().find(|c| c.id == id).map(|c| format!(" ({})", one_line(&c.text))).unwrap_or_default();
    let parts: Vec<String> = effects
        .iter()
        .map(|effect| match effect {
            Effect::AddedClaim { claim } => format!("added {claim}{}", text(claim)),
            Effect::AddedInference { inference } => format!("added {inference}"),
            Effect::RemovedPremise { inference, premise } => format!("removed {premise} from {inference}"),
            Effect::RemovedInference { inference, premises, conclusion } => {
                format!("removed {inference} ({} → {conclusion})", premises.join(", "))
            }
        })
        .collect();
    if parts.is_empty() { String::new() } else { format!(" Effect: {}.", parts.join("; ")) }
}

/// A critique's final status, named by what an acceptance did and by the ground a dropped
/// critique was rejected on.
fn ledger_status(row: &LedgerRow, transcript: &[Turn]) -> String {
    match (row.status, &row.critique.target) {
        (CritiqueStatus::Accepted, Target::Gap { .. }) => "premise added".into(),
        (CritiqueStatus::Accepted, Target::Superfluous { .. }) => "premise removed".into(),
        (CritiqueStatus::Accepted, Target::Circular { .. }) => "patch removed".into(),
        (CritiqueStatus::Accepted, Target::Reading { .. }) => "reading corrected".into(),
        (CritiqueStatus::Adapted, _) => "adapted".into(),
        (CritiqueStatus::Conceded, _) => "conceded".into(),
        (CritiqueStatus::DroppedAfterRejection, _) => {
            let ground = transcript
                .iter()
                .filter_map(|t| match t {
                    Turn::Advocate { turn, .. } => Some(turn.answers.iter()),
                    _ => None,
                })
                .flatten()
                .filter(|a| a.critique == row.critique.id && a.kind == AnswerKind::Reject)
                .filter_map(|a| a.ground)
                .next_back();
            match ground {
                Some(ground) => format!("dropped after rejection: {}", ground_name(ground)),
                None => "dropped after rejection".into(),
            }
        }
        (CritiqueStatus::Open, _) => "open".into(),
        (CritiqueStatus::Unanswered, _) => "unanswered".into(),
    }
}

fn ground_name(ground: Ground) -> &'static str {
    match ground {
        Ground::Stated => "stated",
        Ground::Follows => "follows",
        Ground::CounterFails => "counter_fails",
    }
}

fn kind_label(answer: &Answer) -> String {
    match (answer.kind, answer.ground) {
        (AnswerKind::Accept, _) => "accepted".into(),
        (AnswerKind::Adapt, _) => "adapted".into(),
        (AnswerKind::Concede, _) => "conceded".into(),
        (AnswerKind::Reject, Some(Ground::Stated)) => "rejected: stated".into(),
        (AnswerKind::Reject, Some(Ground::Follows)) => "rejected: follows".into(),
        (AnswerKind::Reject, Some(Ground::CounterFails)) => "rejected: counter_fails".into(),
        (AnswerKind::Reject, None) => "rejected".into(),
    }
}

fn target_phrase(critique: &Critique) -> String {
    format!("{} on {}", critique.target.kind(), critique.target.describe())
}

fn quotes(places: &[String]) -> String {
    places.iter().map(|p| format!("\"{}\"", one_line(p))).collect::<Vec<_>>().join("; ")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One line that ends as a sentence, so the next sentence can follow it.
fn sentence(text: &str) -> String {
    let line = one_line(text);
    match line.chars().last() {
        Some('.' | '!' | '?') | None => line,
        Some(_) => line + ".",
    }
}
