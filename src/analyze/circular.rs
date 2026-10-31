use super::{Analyzer, Finding};
use crate::check::circular_candidates;
use crate::config::CheckConfig;
use crate::model::CheckedTree;

/// Premises that hold most of their conclusion's content words, and so may restate it.
pub struct Circular {
    pub rules: CheckConfig,
}

impl Analyzer for Circular {
    fn name(&self) -> &'static str {
        "circular"
    }

    fn analyze(&self, _proposal: &str, tree: &CheckedTree) -> Vec<Finding> {
        circular_candidates(&tree.tree, &self.rules)
            .into_iter()
            .map(|c| Finding::Circular { inference: c.inference, premise: c.premise, conclusion: c.conclusion, share: c.share })
            .collect()
    }
}
