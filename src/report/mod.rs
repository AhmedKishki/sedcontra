//! The report the caller receives, and its renderings.

mod argdown;
mod markdown;

use std::path::PathBuf;

use serde::Serialize;

use crate::analyze::Finding;
use crate::model::{Change, CheckedTree, Critique, DebateOutcome, Ending, LedgerRow, OpenCritique, Turn};
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
    pub open: Vec<OpenCritique>,
    pub unanswered: Vec<Critique>,
    pub findings: Vec<Finding>,
    pub transcript: Vec<Turn>,
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
        open: outcome.open,
        unanswered: outcome.unanswered,
        findings,
        transcript: outcome.turns,
    }
}
