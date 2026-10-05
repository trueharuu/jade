use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_nav::buffer::Moves;
use jade_nav::fast;

use crate::parse::Saves;

pub const PC_4: Board = Board::lines(4);
pub const PC_2: Board = Board::lines(2);

/// Performs DFS to determine if `queue` can reach the goal from `board` while
/// optionally using hold (if available).
///
/// `queue[0]` is the active piece and hold starts empty. With `hold`, a
/// placement uses the active piece, the hold piece, or (if hold is empty) holds
/// the active piece and places the next one.
///
/// Reaching the goal at any prefix of the queue counts as success.
///
/// A goal is defined to be exactly [`PC_4`] or either [`PC_2`] or [`PC_4`] if
/// `two_l` is true.
///
/// Returns `false` if the cell count of `board` is not a multiple of 4.
/// Panics if `queue.len() > 11`.
///
/// This uses a thread-local [`Solver`]. Own a `Solver` to avoid the lookup.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece], hold: bool) -> bool {
    false
}

/// A thin wrapper that stores a `Board`, an optional hold `Piece`, and the
/// depth among the tree this node inhabits.
///
/// Layout: board bits 0..40, hold bits 40..44 (0 = empty, else piece + 1),
/// depth bits 44..48. Bits 48..64 are zero.
#[repr(transparent)]
pub struct Node(u64);

impl Node {
    pub const fn new(board: Board, hold: Option<Piece>, depth: u8) -> Self {
        let hold = match hold {
            Some(p) => p as u64 + 1,
            None => 0,
        };
        let depth = depth as u64;
        Node(board.0 | (hold << 40) | (depth << 44))
    }

    pub const fn board(&self) -> Board {
        Board(self.0 & ((1 << 40) - 1))
    }

    pub const fn hold(&self) -> Option<Piece> {
        let hold = (self.0 >> 40) & 0xf;
        if hold == 0 {
            None
        } else {
            // Valid by the layout assertion at the top of this file.
            Some(unsafe { std::mem::transmute((hold - 1) as u8) })
        }
    }

    pub const fn depth(&self) -> u8 {
        ((self.0 >> 44) & 0xf) as u8
    }

    /// Returns whether the board can never be pruned to meet any goal.
    pub const fn pruned(&self) -> bool {
        let board = self.board();
        board.has_imbalanced_split() || board.has_isolated_cell()
    }

    pub fn is_goal(&self, two_l: bool) -> bool {
        let board = self.board();
        let is_pc = if two_l {
            board == PC_2 || board == PC_4
        } else {
            board == PC_4
        };
        is_pc
    }
}

#[must_use]
pub fn moves(board: Board, piece: Piece) -> Moves {
    match piece {
        Piece::T => fast::generate::<{ Piece::T }>(&board),
        Piece::I => fast::generate::<{ Piece::I }>(&board),
        Piece::J => fast::generate::<{ Piece::J }>(&board),
        Piece::L => fast::generate::<{ Piece::L }>(&board),
        Piece::O => fast::generate::<{ Piece::O }>(&board),
        Piece::S => fast::generate::<{ Piece::S }>(&board),
        Piece::Z => fast::generate::<{ Piece::Z }>(&board),
    }
}

/// Depth-first search solver for a single queue. This is thread-local and should be used via [`reachable`].
pub struct Search {
    
}