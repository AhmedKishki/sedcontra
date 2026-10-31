//! Cases code finds for the adversary to judge: conclusions no inference supports.

use std::collections::BTreeSet;

use crate::model::{ClaimId, Tree};

/// The conclusions that no inference concludes, apart from reported ones, in tree order.
pub fn unsupported_conclusions(tree: &Tree) -> Vec<ClaimId> {
    let supported: BTreeSet<&str> =
        tree.inferences.iter().filter(|i| !i.reported).map(|i| i.conclusion.as_str()).collect();
    tree.conclusions.iter().filter(|c| !supported.contains(c.as_str())).cloned().collect()
}
