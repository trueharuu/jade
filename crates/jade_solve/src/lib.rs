//! Boolean Perfect-Clear solver.
//! Solver.

pub mod congruents;
pub mod evaluate;
pub mod fumen;
pub mod moves;
pub mod percent;
pub mod rules;
pub mod saves;
pub mod setup;
pub mod solve;

/// Parse fumen.
pub use fumen::parse_fumen;
/// Saves.
pub use saves::Saves;
