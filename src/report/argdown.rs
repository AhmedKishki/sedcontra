//! The final tree as Argdown premise–conclusion structures.

use std::collections::BTreeSet;

use crate::model::{Claim, Tree};

pub fn render_argdown(tree: &Tree) -> String {
    let conclusions: BTreeSet<&str> = tree.conclusions.iter().map(String::as_str).collect();
    let statement = |id: &str| -> String {
        match tree.claims.iter().find(|c| c.id == id) {
            Some(claim) => format!("[{}]: {}", claim.id, text_with_tags(claim, conclusions.contains(id))),
            None => format!("[{id}]"),
        }
    };

    let mut blocks = Vec::new();
    for inference in &tree.inferences {
        let mut lines = vec![format!("<{}>", inference.id)];
        for (n, premise) in inference.premises.iter().enumerate() {
            lines.push(format!("({}) {}", n + 1, statement(premise)));
        }
        lines.push("-----".into());
        lines.push(format!("({}) {}", inference.premises.len() + 1, statement(&inference.conclusion)));
        blocks.push(lines.join("\n"));
    }

    let used: BTreeSet<&str> = tree
        .inferences
        .iter()
        .flat_map(|i| i.premises.iter().chain([&i.conclusion]))
        .map(String::as_str)
        .collect();
    let standalone: Vec<String> = tree
        .claims
        .iter()
        .filter(|c| !used.contains(c.id.as_str()))
        .map(|c| statement(&c.id))
        .collect();
    if !standalone.is_empty() {
        blocks.push(standalone.join("\n"));
    }

    blocks.join("\n\n") + "\n"
}

fn text_with_tags(claim: &Claim, is_conclusion: bool) -> String {
    let mut text = sanitise(&claim.text);
    if is_conclusion {
        text.push_str(" #conclusion");
    }
    if claim.origin.is_some() {
        text.push_str(" #added");
    }
    if claim.backed_by_context {
        text.push_str(" #backed-by-context");
    }
    text
}

/// Replaces characters Argdown would read as syntax.
fn sanitise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("//", "/ /")
        .chars()
        .map(|c| match c {
            '[' | '{' => '(',
            ']' | '}' => ')',
            '<' => '‹',
            '>' => '›',
            '#' => '＃',
            other => other,
        })
        .collect()
}
