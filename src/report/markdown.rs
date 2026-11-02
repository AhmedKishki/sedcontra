//! The report as markdown, for an agent or a person to read.

use std::fmt::Write;

use super::Report;
use crate::model::{
    AdversaryTurn, AdvocateTurn, Answer, AnswerKind, CritiqueStatus, Ending, Ground, Reply, ResponseAction, Thread, Turn,
};

pub fn render_markdown(report: &Report) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# Cross-examination report\n");
    let _ = writeln!(out, "Run `{}`, record in `{}`.\n", report.run_id, report.record_dir.display());

    let _ = writeln!(out, "## Ending\n");
    let open = report.threads.iter().filter(|t| t.status == CritiqueStatus::Open).count();
    let ending = match report.ending {
        Ending::Satisfied { round } => format!(
            "The adversary was satisfied in round {round} of at most {}: it had nothing to press and nothing to raise.",
            report.round_limit
        ),
        Ending::LimitReached if open == 0 => {
            format!("The round limit of {} was reached with every critique settled.", report.round_limit)
        }
        Ending::LimitReached => format!(
            "The round limit of {} was reached with {open} open {}, which both sides argued and neither yielded.",
            report.round_limit,
            if open == 1 { "critique" } else { "critiques" }
        ),
    };
    let _ = writeln!(out, "{ending}\n\nThis describes how the debate ended. It is not a verdict on the proposal.\n");

    let _ = writeln!(out, "## Critique ledger\n");
    if report.threads.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Critique | Kind | Place | Raised in | Status |\n| --- | --- | --- | --- | --- |");
        for thread in &report.threads {
            let place = thread.critique.places.first().map(|p| shorten(p)).unwrap_or_default();
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                thread.critique.id,
                thread.critique.kind,
                place,
                thread.raised_in,
                status_label(thread.status)
            );
        }
        out.push('\n');
    }

    let sections = [
        (CritiqueStatus::Accepted, "Accepted critiques"),
        (CritiqueStatus::Dropped, "Dropped critiques"),
        (CritiqueStatus::Open, "Open critiques"),
    ];
    for (status, heading) in sections {
        let _ = writeln!(out, "## {heading}\n");
        let threads: Vec<&Thread> = report.threads.iter().filter(|t| t.status == status).collect();
        if threads.is_empty() {
            let _ = writeln!(out, "None.\n");
        }
        for thread in threads {
            render_thread(&mut out, thread);
        }
    }

    let _ = writeln!(out, "## Transcript\n");
    out.push_str(&render_transcript(&report.transcript));
    out
}

fn render_thread(out: &mut String, thread: &Thread) {
    let critique = &thread.critique;
    let _ = writeln!(out, "### {} ({}), raised in round {}\n", critique.id, critique.kind, thread.raised_in);
    for place in &critique.places {
        let _ = writeln!(out, "> {}\n", one_line(place));
    }
    let _ = writeln!(out, "{}\n\nSupport: {}\n", sentence(&critique.point), one_line(&critique.support));
    for reply in &thread.replies {
        match reply {
            Reply::Advocate { round, answer } => {
                let _ = writeln!(out, "- Advocate, round {round}, {}: {}", answer_label(answer), one_line(&answer.reasons));
                write_quotes(out, &answer.quotes);
            }
            Reply::Adversary { round, response } => {
                let _ = writeln!(
                    out,
                    "- Adversary, round {round}, {}: {}",
                    action_label(response.action),
                    one_line(&response.reasons)
                );
                write_quotes(out, &response.quotes);
            }
        }
    }
    out.push('\n');
}

pub fn render_transcript(turns: &[Turn]) -> String {
    let mut out = String::new();
    for turn in turns {
        let _ = writeln!(out, "### Round {}: {}\n", turn.round(), turn.role());
        match turn {
            Turn::Adversary { turn, .. } => render_adversary(&mut out, turn),
            Turn::Advocate { turn, .. } => render_advocate(&mut out, turn),
        }
        out.push('\n');
    }
    out
}

fn render_adversary(out: &mut String, turn: &AdversaryTurn) {
    for response in &turn.responses {
        let _ = writeln!(out, "- **{}** {}: {}", response.critique, action_label(response.action), one_line(&response.reasons));
        write_quotes(out, &response.quotes);
    }
    for critique in &turn.critiques {
        let places: Vec<String> = critique.places.iter().map(|p| format!("“{}”", one_line(p))).collect();
        let _ = writeln!(out, "- **{}** ({}) at {}: {}", critique.id, critique.kind, places.join(", "), sentence(&critique.point));
        let _ = writeln!(out, "  Support: {}", one_line(&critique.support));
    }
    let _ = writeln!(out, "- Satisfied: {}", if turn.satisfied { "yes" } else { "no" });
}

fn render_advocate(out: &mut String, turn: &AdvocateTurn) {
    for answer in &turn.answers {
        let _ = writeln!(out, "- **{}** {}: {}", answer.critique, answer_label(answer), one_line(&answer.reasons));
        write_quotes(out, &answer.quotes);
    }
}

fn write_quotes(out: &mut String, quotes: &[String]) {
    for quote in quotes {
        let _ = writeln!(out, "  > {}", one_line(quote));
    }
}

fn status_label(status: CritiqueStatus) -> &'static str {
    match status {
        CritiqueStatus::Accepted => "accepted",
        CritiqueStatus::Dropped => "dropped",
        CritiqueStatus::Open => "open",
    }
}

fn action_label(action: ResponseAction) -> &'static str {
    match action {
        ResponseAction::Drop => "dropped",
        ResponseAction::Press => "pressed",
    }
}

fn answer_label(answer: &Answer) -> &'static str {
    match (answer.kind, answer.ground) {
        (AnswerKind::Accept, _) => "accepted",
        (AnswerKind::Discuss, Some(Ground::Misreads)) => "discussed (misreads)",
        (AnswerKind::Discuss, Some(Ground::Unclear)) => "discussed (unclear)",
        (AnswerKind::Discuss, None) => "discussed",
    }
}

/// The first place of a critique, cut for the ledger table; the sections quote it in full.
fn shorten(text: &str) -> String {
    let line = one_line(text);
    if line.chars().count() > 80 { line.chars().take(79).collect::<String>() + "…" } else { line }
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
