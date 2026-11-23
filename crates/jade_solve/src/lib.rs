//! Boolean Perfect-Clear solver.
pub mod best;
pub mod congruents;
pub mod evaluate;
pub mod fumen;
pub mod moves;
pub mod percent;
pub mod rules;
pub mod saves;
pub mod setup;
pub mod solve;

pub use best::BestResult;
pub use best::best;
pub use fumen::parse_fumen;
pub use saves::Saves;