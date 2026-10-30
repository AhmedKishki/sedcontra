//! Code checks on what the advocate and the adversary return.

pub mod quotes;
pub mod tree;
pub mod turn;

pub use quotes::QuoteIndex;
pub use tree::check_tree;
pub use turn::{check_adversary_turn, check_advocate_turn};
