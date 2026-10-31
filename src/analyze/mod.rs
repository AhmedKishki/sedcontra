//! Structural findings, computed in code from the final tree and the proposal, and the support
//! of each conclusion, computed from the final tree and the ledger.

mod circular;
mod restatement;
mod stub;
mod support;
mod uncovered;
mod unsupported;

use serde::{Deserialize, Serialize};

use crate::config::{AnalyzerConfig, CheckConfig};
use crate::model::{CheckedTree, ClaimId, InferenceId, Span};

pub use circular::Circular;
pub use restatement::Restatement;
pub use stub::Stubs;
pub use support::{ConclusionSupport, conclusion_support};
pub use uncovered::Uncovered;
pub use unsupported::Unsupported;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    Restatement { claim: ClaimId, quotes: Vec<String>, spans: Vec<Span> },
    Uncovered { text: String, span: Span },
    Stub { claims: Vec<ClaimId>, prune: bool },
    Unsupported { claim: ClaimId },
    Circular { inference: InferenceId, premise: ClaimId, conclusion: ClaimId, share: f64 },
}

pub trait Analyzer: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze(&self, proposal: &str, tree: &CheckedTree) -> Vec<Finding>;
}

pub fn default_analyzers(analyzers: &AnalyzerConfig, checks: &CheckConfig) -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(Restatement),
        Box::new(Uncovered { min_words: analyzers.uncovered_min_words }),
        Box::new(Stubs { max_len: analyzers.stub_max_len }),
        Box::new(Unsupported),
        Box::new(Circular { rules: checks.clone() }),
    ]
}
