//! Cases code finds for the adversary to judge: conclusions no inference supports, and premises
//! that may restate their conclusion.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::quotes::words;
use crate::config::CheckConfig;
use crate::model::{ClaimId, InferenceId, Tree};

/// The conclusions that no inference concludes, apart from reported ones, in tree order.
pub fn unsupported_conclusions(tree: &Tree) -> Vec<ClaimId> {
    let supported: BTreeSet<&str> =
        tree.inferences.iter().filter(|i| !i.reported).map(|i| i.conclusion.as_str()).collect();
    tree.conclusions.iter().filter(|c| !supported.contains(c.as_str())).cloned().collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CircularCandidate {
    pub inference: InferenceId,
    pub premise: ClaimId,
    pub conclusion: ClaimId,
    /// The share of the conclusion's content words that the premise holds.
    pub share: f64,
}

/// The premises of unreported inferences that hold at least the configured share of their
/// conclusion's content words, the words not in the stopword list.
pub fn circular_candidates(tree: &Tree, rules: &CheckConfig) -> Vec<CircularCandidate> {
    let stopwords: BTreeSet<String> = rules.stopwords.iter().map(|w| w.replace('’', "'").to_lowercase()).collect();
    let content = |id: &str| -> Option<BTreeSet<String>> {
        let claim = tree.claims.iter().find(|c| c.id == id)?;
        Some(words(&claim.text).into_iter().filter(|w| !stopwords.contains(w)).collect())
    };
    let mut found = Vec::new();
    for inference in tree.inferences.iter().filter(|i| !i.reported) {
        let Some(conclusion) = content(&inference.conclusion).filter(|c| !c.is_empty()) else {
            continue;
        };
        for premise in &inference.premises {
            let Some(held) = content(premise) else { continue };
            let share = conclusion.intersection(&held).count() as f64 / conclusion.len() as f64;
            if share >= rules.circular_share {
                found.push(CircularCandidate {
                    inference: inference.id.clone(),
                    premise: premise.clone(),
                    conclusion: inference.conclusion.clone(),
                    share,
                });
            }
        }
    }
    found
}
