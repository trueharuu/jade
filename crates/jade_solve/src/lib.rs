//! Boolean Perfect-Clear solver.
//!
//! Given an initial board and a concrete queue, decides whether some legal
//! placement sequence reaches the full-board state `header::PC_4` while
//! using hold.

pub mod solve;

pub use solve::reachable;