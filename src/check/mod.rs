//! Code checks on what the advocate and the adversary return.

pub mod candidates;
pub mod compare;
pub mod quotes;
pub mod tree;
pub mod turn;

pub use candidates::{CircularCandidate, circular_candidates, unsupported_conclusions};
pub use compare::{TreeDiff, check_revision, diff_trees, touches};
pub use quotes::QuoteIndex;
pub use tree::check_tree;
pub use turn::{check_adversary_turn, check_advocate_turn};
