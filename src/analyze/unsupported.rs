use super::{Analyzer, Finding};
use crate::check::unsupported_conclusions;
use crate::model::CheckedTree;

/// Conclusions that no inference concludes, apart from reported ones.
pub struct Unsupported;

impl Analyzer for Unsupported {
    fn name(&self) -> &'static str {
        "unsupported"
    }

    fn analyze(&self, _proposal: &str, tree: &CheckedTree) -> Vec<Finding> {
        unsupported_conclusions(&tree.tree).into_iter().map(|claim| Finding::Unsupported { claim }).collect()
    }
}
