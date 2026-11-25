//! Highest-scoring path search.
//!
//! Given a starting board and one queue, this module enumerates every
//! perfect-clear solution, scores each one, and keeps the highest-scoring
//! move sequence together with its scoring events.

pub mod event;
pub mod score;
pub mod search;

use jade_core::board::Board;
use jade_core::list::List;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::CAP;

use crate::event::Event;
use jade_solve::Saves;
use score::CELLS_PER_PIECE;
use score::PC_CELLS;

/// The highest-scoring solution found by [`best_with_options`].
#[derive(Debug, Clone, Copy)]
pub struct BestResult {
    /// The move sequence, in placement order.
    pub moves: List<Move, CAP>,
    /// The scoring event for each move, in the same order.
    pub events: List<Event, CAP>,
    /// Total score of the sequence.
    pub score: u32,
}

/// Search settings.
///
/// Build with [`Options::new`]. Use struct update syntax to set the
/// initial state:
/// `Options { initial_hold: Some(p), ..Options::new(two_l, saves, true) }`.
#[derive(Clone, Copy)]
pub struct Options {
    /// Accept the 2-line goal board as well as the 4-line goal board.
    pub two_l: bool,
    pub saves: Saves,
    pub hold_enabled: bool,
    /// Ignored when `hold_enabled` is `false`.
    pub initial_hold: Option<Piece>,
    pub initial_b2b: bool,
    pub initial_combo: Option<u32>,
    /// When true, non-perfect-clear paths may be returned as partial results.
    pub partial: bool,
}

impl Options {
    #[must_use]
    pub const fn new(two_l: bool, saves: Saves, hold_enabled: bool) -> Self {
        Self {
            two_l,
            saves,
            hold_enabled,
            initial_hold: None,
            initial_b2b: false,
            initial_combo: None,
            partial: false,
        }
    }

    #[must_use]
    pub fn best(&self, board: Board, queue: &[Piece]) -> Option<BestResult> {
        let board = board.clearshift();
        let cells = board.popcount();
        if !cells.is_multiple_of(CELLS_PER_PIECE) || cells > PC_CELLS {
            return None;
        }
        search::run(board, queue, *self)
    }
}
