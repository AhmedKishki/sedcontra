//! The report the caller receives, and its renderings.

mod markdown;

use std::path::PathBuf;

use serde::Serialize;

use crate::model::{DebateOutcome, Ending, Thread, Turn};
use crate::record::RunRecord;

pub use markdown::{render_markdown, render_transcript};

#[derive(Serialize, Clone, Debug)]
pub struct Report {
    pub run_id: String,
    pub record_dir: PathBuf,
    pub ending: Ending,
    pub rounds_run: u32,
    pub round_limit: u32,
    pub threads: Vec<Thread>,
    pub transcript: Vec<Turn>,
}

pub fn build_report(record: &RunRecord, outcome: DebateOutcome) -> Report {
    Report {
        run_id: record.id.clone(),
        record_dir: record.dir.clone(),
        ending: outcome.ending,
        rounds_run: outcome.rounds_run,
        round_limit: outcome.round_limit,
        threads: outcome.threads,
        transcript: outcome.turns,
    }
}
