use std::collections::{BTreeMap, BTreeSet};

use super::{Analyzer, Finding};
use crate::model::CheckedTree;

/// Groups of claims from which no inference path reaches a conclusion.
pub struct Stubs {
    pub max_len: usize,
}

impl Analyzer for Stubs {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn analyze(&self, _proposal: &str, tree: &CheckedTree) -> Vec<Finding> {
        let tree = &tree.tree;

        // A claim is on the argument when it is a conclusion or a premise of an inference
        // whose conclusion is on the argument.
        let mut on: BTreeSet<&str> = tree.conclusions.iter().map(String::as_str).collect();
        loop {
            let before = on.len();
            for inference in &tree.inferences {
                if on.contains(inference.conclusion.as_str()) {
                    on.extend(inference.premises.iter().map(String::as_str));
                }
            }
            if on.len() == before {
                break;
            }
        }

        let off: Vec<&str> =
            tree.claims.iter().map(|c| c.id.as_str()).filter(|id| !on.contains(id)).collect();
        let position: BTreeMap<&str, usize> = off.iter().enumerate().map(|(i, id)| (*id, i)).collect();

        // Union-find over the claims off the argument, joined by the inferences among them.
        let mut parent: Vec<usize> = (0..off.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for inference in &tree.inferences {
            let members: Vec<usize> = inference
                .premises
                .iter()
                .chain([&inference.conclusion])
                .filter_map(|id| position.get(id.as_str()).copied())
                .collect();
            for pair in members.windows(2) {
                let (a, b) = (root(&mut parent, pair[0]), root(&mut parent, pair[1]));
                parent[a] = b;
            }
        }

        let mut groups: Vec<(usize, Vec<String>)> = Vec::new();
        for (i, id) in off.iter().enumerate() {
            let r = root(&mut parent, i);
            match groups.iter_mut().find(|(gr, _)| *gr == r) {
                Some((_, claims)) => claims.push(id.to_string()),
                None => groups.push((r, vec![id.to_string()])),
            }
        }
        groups
            .into_iter()
            .map(|(_, claims)| {
                let prune = claims.len() <= self.max_len;
                Finding::Stub { claims, prune }
            })
            .collect()
    }
}
