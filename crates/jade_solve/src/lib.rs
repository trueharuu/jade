//! Boolean Perfect-Clear solver.
//!
//! Given an initial board and a concrete queue, decides whether some legal
//! placement sequence reaches the full-board state `header::PC_4` while
//! using hold.
//!
//! The `reachable_save` functions add a second condition: the hold must hold
//! a piece whose type is in the given save set at the moment the field fills.

pub mod parse;
pub mod solve;

pub use parse::parse_fumen;
pub use solve::is_unfillable;
pub use solve::reachable;
pub use solve::reachable_2l;
pub use solve::reachable_save;
pub use solve::reachable_save_2l;
