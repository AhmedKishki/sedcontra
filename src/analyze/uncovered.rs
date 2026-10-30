use super::{Analyzer, Finding};
use crate::model::{CheckedTree, Span};

/// Stretches of the proposal that no place quotes.
pub struct Uncovered {
    pub min_words: usize,
}

impl Analyzer for Uncovered {
    fn name(&self) -> &'static str {
        "uncovered"
    }

    fn analyze(&self, proposal: &str, tree: &CheckedTree) -> Vec<Finding> {
        let mut covered = vec![false; proposal.len()];
        for span in tree.claim_spans.values().chain(tree.inference_spans.values()).flatten() {
            covered[span.start..span.end].fill(true);
        }

        let mut findings = Vec::new();
        let mut start = None;
        for i in 0..=proposal.len() {
            let is_covered = covered.get(i).copied().unwrap_or(true);
            match (start, is_covered) {
                (None, false) => start = Some(i),
                (Some(s), true) => {
                    self.report(proposal, s, i, &mut findings);
                    start = None;
                }
                _ => {}
            }
        }
        findings
    }
}

impl Uncovered {
    fn report(&self, proposal: &str, start: usize, end: usize, findings: &mut Vec<Finding>) {
        let stretch = &proposal[start..end];
        let text = stretch.trim();
        let words = text
            .split_whitespace()
            .filter(|w| w.chars().any(char::is_alphanumeric))
            .count();
        if words < self.min_words {
            return;
        }
        let lead = stretch.len() - stretch.trim_start().len();
        let span = Span { start: start + lead, end: start + lead + text.len() };
        findings.push(Finding::Uncovered { text: text.to_string(), span });
    }
}
