//! Loads the cases of an evaluation, which are proposals with defects planted at quoted places, and scores what a debate did with each defect.

pub mod case;
pub mod score;

pub use case::{Case, Defect, Planted, load_cases};
pub use score::{DefectScore, Landing, Outcome, RunScore, score};
