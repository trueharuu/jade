//! Spin classification.

/// Spin type of a placed piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spin {
    /// No spin.
    None,
    /// Mini spin.
    Mini,
    /// Full spin.
    Full,
}
