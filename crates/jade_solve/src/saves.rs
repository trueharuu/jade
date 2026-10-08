//! The set of pieces that may be left over when the field fills.

use std::fmt::Display;
use std::str::FromStr;

use jade_core::piece::Piece;

/// A set of piece types, stored as one bit per piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saves(u8);

impl Saves {
    /// Returns a set that allows every leftover piece.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Saves(0)
    }

    /// Returns a set that contains every piece type.
    #[inline]
    #[must_use]
    pub const fn all() -> Self {
        Saves((1 << Piece::ALL.len()) - 1)
    }

    /// Returns the number of pieces in the set.
    #[inline]
    #[must_use]
    pub const fn size(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns whether the set is empty. An empty set is no restriction.
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether `piece` is in the set.
    #[inline]
    #[must_use]
    pub const fn has(&self, piece: Piece) -> bool {
        self.0 & (1 << (piece as u8)) != 0
    }

    /// Adds `piece` to the set.
    #[inline]
    pub fn add(&mut self, piece: Piece) {
        self.0 |= 1 << (piece as u8);
    }

    /// Removes `piece` from the set, if present.
    #[inline]
    pub fn remove(&mut self, piece: Piece) {
        self.0 &= !(1 << (piece as u8));
    }
}

impl FromStr for Saves {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut saves = 0;
        for c in s.chars() {
            let piece =
                Piece::from_str(&c.to_string()).map_err(|_| format!("invalid piece: {c}"))?;
            saves |= 1 << (piece as u8);
        }
        Ok(Saves(saves))
    }
}

impl Display for Saves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for piece in Piece::ALL {
            if self.has(piece) {
                write!(f, "{piece}")?;
            }
        }

        Ok(())
    }
}

/// Returns whether the leftover piece satisfies `saves`.
///
/// An empty set means no restriction. Otherwise the leftover is the hold piece,
/// or the active piece if hold is empty. No leftover fails a non-empty set.
#[inline(always)]
#[must_use]
pub(crate) fn save_ok(saves: Saves, hold: Option<Piece>, active: Option<Piece>) -> bool {
    if saves.is_empty() {
        return true;
    }
    match hold.or(active) {
        Some(p) => saves.has(p),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_text() {
        for src in ["", "T", "TIJL", "Z", "TIJLOSZ"] {
            let saves: Saves = src.parse().unwrap();
            assert_eq!(saves.to_string(), src);
        }
    }

    #[test]
    fn empty_is_no_restriction() {
        let saves = Saves::empty();
        assert!(saves.is_empty());
        assert_eq!(saves.size(), 0);
        // An empty set accepts anything, including no leftover at all.
        assert!(save_ok(saves, None, None));
        assert!(save_ok(saves, Some(Piece::T), None));
    }

    #[test]
    fn nonempty_needs_a_matching_leftover() {
        let saves: Saves = "T".parse().unwrap();
        assert!(save_ok(saves, Some(Piece::T), None));
        assert!(save_ok(saves, None, Some(Piece::T)));
        // Hold wins over active.
        assert!(!save_ok(saves, Some(Piece::I), Some(Piece::T)));
        assert!(!save_ok(saves, None, Some(Piece::I)));
        // A nonempty set with no leftover always fails.
        assert!(!save_ok(saves, None, None));
    }

    #[test]
    fn rejects_bad_piece_char() {
        assert!("X".parse::<Saves>().is_err());
    }
}
