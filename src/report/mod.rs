//! The report the caller receives, and its renderings.

mod argdown;
mod markdown;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::analyze::Finding;
use crate::model::{
    Change, CheckedTree, Critique, CritiqueId, DebateOutcome, Ending, InferenceId, LedgerRow, OpenCritique, Turn,
};
use crate::record::RunRecord;

pub use argdown::render_argdown;
pub use markdown::{render_markdown, render_transcript};

#[derive(Serialize, Clone, Debug)]
pub struct Report {
    pub run_id: String,
    pub record_dir: PathBuf,
    pub ending: Ending,
    pub rounds_run: u32,
    pub round_limit: u32,
    pub ledger: Vec<LedgerRow>,
    pub final_tree: CheckedTree,
    pub changes: Vec<Change>,
    pub conceded: Vec<Change>,
    pub closed: Vec<Change>,
    pub open: Vec<OpenCritique>,
    pub unanswered: Vec<Critique>,
    pub findings: Vec<Finding>,
    pub transcript: Vec<Turn>,
}

impl Report {
    /// Each conceded inference with the critiques conceded against it.
    pub fn conceded_by_inference(&self) -> BTreeMap<InferenceId, Vec<CritiqueId>> {
        let mut by_inference: BTreeMap<InferenceId, Vec<CritiqueId>> = BTreeMap::new();
        for change in &self.conceded {
            if let Some(inference) = change.critique.target.inference() {
                by_inference.entry(inference.to_string()).or_default().push(change.critique.id.clone());
            }
        }
        by_inference
    }
}

pub fn build_report(record: &RunRecord, outcome: DebateOutcome, findings: Vec<Finding>) -> Report {
    Report {
        run_id: record.id.clone(),
        record_dir: record.dir.clone(),
        ending: outcome.ending,
        rounds_run: outcome.rounds_run,
        round_limit: outcome.round_limit,
        ledger: outcome.ledger,
        final_tree: outcome.final_tree,
        changes: outcome.changes,
        conceded: outcome.conceded,
        closed: outcome.closed,
        open: outcome.open,
        unanswered: outcome.unanswered,
        findings,
        transcript: outcome.turns,
    }
}
