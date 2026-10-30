use super::{Analyzer, Finding};
use crate::model::CheckedTree;

/// A claim the proposal states in more than one place.
pub struct Restatement;

impl Analyzer for Restatement {
    fn name(&self) -> &'static str {
        "restatement"
    }

    fn analyze(&self, _proposal: &str, tree: &CheckedTree) -> Vec<Finding> {
        tree.tree
            .claims
            .iter()
            .filter_map(|claim| {
                let spans = tree.claim_spans.get(&claim.id)?;
                (spans.len() > 1).then(|| Finding::Restatement {
                    claim: claim.id.clone(),
                    quotes: claim.places.clone(),
                    spans: spans.clone(),
                })
            })
            .collect()
    }
}
