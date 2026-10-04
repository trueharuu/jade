use std::cell::RefCell;
use std::fmt::Display;
use std::str::FromStr;

use jade_core::board::Board;
use jade_core::header::PLAY_LINES;
use jade_core::header::rows_below;
use jade_core::piece::Piece;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saves(u8);

impl Saves {
    // piece is from 0..=7
    pub const fn has(&self, piece: Piece) -> bool {
        self.0 & (1 << (piece as u8)) != 0
    }
}

impl FromStr for Saves {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut saves = 0;
        for c in s.chars() {
            let piece = Piece::from_str(&c.to_string()).map_err(|_| format!("invalid piece: {c}"))?;
            saves |= 1 << (piece as u8);
        }
        Ok(Saves(saves))
    }
}

impl Display for Saves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for piece in [Piece::T, Piece::I, Piece::J, Piece::L, Piece::O, Piece::S, Piece::Z] {
            if self.has(piece) {
                write!(f, "{piece}")?;
            }
        }

        Ok(())
    }
}

/// Performs DFS to determine if `queue` can reach the `goal` from `board` while optionally using hold (if available).
///
/// Reaching `goal` at any prefix of the queue counts as success.
/// When `saves` is nonzero, the remaining hold of the search when attaining `goal` must be within that set.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece], goal: Board, saves: u8, hold: bool) -> bool {
    false
}