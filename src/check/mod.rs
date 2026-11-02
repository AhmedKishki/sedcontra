//! Code checks on what the adversary and the advocate return.

pub mod quotes;
pub mod turn;

pub use quotes::QuoteIndex;
pub use turn::{check_adversary_turn, check_advocate_turn};
