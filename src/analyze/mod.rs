//! Structural findings, computed in code from the final tree and the proposal.

mod restatement;
mod stub;
mod uncovered;

use serde::{Deserialize, Serialize};

use crate::config::AnalyzerConfig;
use crate::model::{CheckedTree, ClaimId, Span};

pub use restatement::Restatement;
pub use stub::Stubs;
pub use uncovered::Uncovered;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    Restatement { claim: ClaimId, quotes: Vec<String>, spans: Vec<Span> },
    Uncovered { text: String, span: Span },
    Stub { claims: Vec<ClaimId>, prune: bool },
}

pub trait Analyzer: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze(&self, proposal: &str, tree: &CheckedTree) -> Vec<Finding>;
}

pub fn default_analyzers(cfg: &AnalyzerConfig) -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(Restatement),
        Box::new(Uncovered { min_words: cfg.uncovered_min_words }),
        Box::new(Stubs { max_len: cfg.stub_max_len }),
    ]
}
