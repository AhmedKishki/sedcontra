//! Loads the cases of an evaluation, which are proposals with defects planted at quoted places, scores what a debate did with each defect and runs every case several times.

pub mod case;
pub mod run;
pub mod score;

pub use case::{Case, Defect, Planted, load_cases};
pub use run::{CaseSummary, DefectSummary, RunResult, Scores, evaluate, render_cases, render_scores};
pub use score::{DefectScore, Landing, Outcome, RunScore, score};
