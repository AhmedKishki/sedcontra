use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{ClaimId, CritiqueId, CritiqueStatus, Inference, InferenceId, LedgerRow, Target, Tree};

/// What a conclusion's support rests on, and which concessions and critiques it passes through.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConclusionSupport {
    pub conclusion: ClaimId,
    /// An inference other than a reported one concludes it.
    pub supported: bool,
    /// Some line of support avoids every conceded inference.
    pub intact: bool,
    pub added: Vec<ClaimId>,
    pub conceded: Vec<InferenceId>,
    pub open: Vec<CritiqueId>,
    pub unanswered: Vec<CritiqueId>,
}

pub fn conclusion_support(tree: &Tree, ledger: &[LedgerRow]) -> Vec<ConclusionSupport> {
    let inferences: Vec<&Inference> = tree.inferences.iter().filter(|i| !i.reported).collect();
    let conceded: BTreeSet<&str> = ledger
        .iter()
        .filter(|r| r.status == CritiqueStatus::Conceded)
        .filter_map(|r| r.critique.target.inference())
        .collect();
    let mut memo: BTreeMap<&str, bool> = BTreeMap::new();

    let mut found = Vec::new();
    for conclusion in &tree.conclusions {
        let support = support_of(conclusion, &inferences);
        let used: Vec<&Inference> = inferences.iter().copied().filter(|i| support.contains(i.id.as_str())).collect();
        let premises: BTreeSet<&str> = used.iter().flat_map(|i| i.premises.iter().map(String::as_str)).collect();
        let claims: BTreeSet<&str> = premises.iter().copied().chain(used.iter().map(|i| i.conclusion.as_str())).collect();
        let reaches = |row: &LedgerRow| match &row.critique.target {
            Target::Reading { of } => support.contains(of.as_str()) || claims.contains(of.as_str()),
            target => target.inference().is_some_and(|i| support.contains(i)),
        };
        let with = |status: CritiqueStatus| -> Vec<CritiqueId> {
            ledger.iter().filter(|r| r.status == status && reaches(r)).map(|r| r.critique.id.clone()).collect()
        };
        let supported = inferences.iter().any(|i| &i.conclusion == conclusion);
        found.push(ConclusionSupport {
            conclusion: conclusion.clone(),
            supported,
            intact: supported && intact(conclusion, &inferences, &conceded, &mut memo),
            added: tree.claims.iter().filter(|c| c.is_added() && premises.contains(c.id.as_str())).map(|c| c.id.clone()).collect(),
            conceded: support.iter().filter(|i| conceded.contains(*i)).map(|i| i.to_string()).collect(),
            open: with(CritiqueStatus::Open),
            unanswered: with(CritiqueStatus::Unanswered),
        });
    }
    found
}

/// The inferences reached by walking back from a claim through the premises of each inference
/// that concludes a claim already reached.
fn support_of<'a>(claim: &'a str, inferences: &[&'a Inference]) -> BTreeSet<&'a str> {
    let mut support = BTreeSet::new();
    let mut reached = BTreeSet::new();
    let mut pending = vec![claim];
    while let Some(next) = pending.pop() {
        if !reached.insert(next) {
            continue;
        }
        for inference in inferences.iter().filter(|i| i.conclusion == next) {
            if support.insert(inference.id.as_str()) {
                pending.extend(inference.premises.iter().map(String::as_str));
            }
        }
    }
    support
}

/// A claim is intact when no inference concludes it, so the argument starts from it, or when
/// some unconceded inference concluding it has only intact premises. The checks rule out loops,
/// so the recursion ends.
fn intact<'a>(claim: &'a str, inferences: &[&'a Inference], conceded: &BTreeSet<&str>, memo: &mut BTreeMap<&'a str, bool>) -> bool {
    if let Some(&known) = memo.get(claim) {
        return known;
    }
    let concluding: Vec<&'a Inference> = inferences.iter().copied().filter(|i| i.conclusion == claim).collect();
    let result = concluding.is_empty()
        || concluding.iter().any(|i| {
            !conceded.contains(i.id.as_str()) && i.premises.iter().all(|p| intact(p, inferences, conceded, memo))
        });
    memo.insert(claim, result);
    result
}
