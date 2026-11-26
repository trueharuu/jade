//! The rules that decide whether a board can reach a goal.
//!
//! Both solvers in this crate call these functions. A rule must be written
//! here once, not in each solver.

use jade_core::board::Board;
use jade_core::piece::Piece;

use crate::moves::moves;

/// The goal board for a full perfect clear.
pub const PC_4: Board = Board::lines(4);
/// The goal board for a two-line clear.
pub const PC_2: Board = Board::lines(2);

/// `Node::hold` transmutes `value - 1` to `Piece`. This is valid only for a
/// one-byte enum with discriminants 0..=6.
const _: () = {
    assert!(std::mem::size_of::<Piece>() == 1);
    assert!((Piece::T as u8) < 7);
    assert!((Piece::I as u8) < 7);
    assert!((Piece::J as u8) < 7);
    assert!((Piece::L as u8) < 7);
    assert!((Piece::O as u8) < 7);
    assert!((Piece::S as u8) < 7);
    assert!((Piece::Z as u8) < 7);
};

/// Converts an index (`piece as u8`) back to a `Piece`. `i` must be below 7.
#[inline(always)]
#[must_use]
pub const fn piece_from_index(i: u8) -> Piece {
    debug_assert!(i < 7);
    // Valid by the layout assertion above.
    unsafe { std::mem::transmute(i) }
}

/// Returns whether the board can never reach any goal.
#[inline(always)]
#[must_use]
pub const fn pruned(board: Board) -> bool {
    board.has_imbalanced_split() || board.has_isolated_cell()
}

/// Returns whether the board has a cell outside the play field.
///
/// Cells at or above bit 40 are outside the four play rows. The search must
/// not descend into them.
#[inline(always)]
#[must_use]
pub const fn outside_playfield(board: Board) -> bool {
    board.0 >> 40 != 0
}

/// Returns how many pieces a board with `cells` filled cells needs to reach a
/// goal.
///
/// This is the count bound. It assumes `cells` is a multiple of four and that
/// the board is in reach, so it can return a count for a goal that the board
/// cannot actually reach.
///
/// `two_l` selects the two-line goal while the board has fewer than 20 cells.
/// At exactly 20 cells the two-line goal is already met and the full goal is
/// the only one left, so the `cells < 20` test is required. Callers that need
/// to know whether a goal is in reach must test that separately.
#[inline(always)]
#[must_use]
pub const fn pieces_to_goal(cells: u32, two_l: bool) -> usize {
    if two_l && cells < 20 {
        ((20 - cells) / 4) as usize
    } else {
        ((40 - cells) / 4) as usize
    }
}

/// Returns whether placing `piece` on `board` reaches `goal`.
///
/// This is the last step of a search. The placement must satisfy `saves`, so
/// the caller must check that first.
#[must_use]
pub fn completes_goal(board: Board, piece: Piece, goal: Board) -> bool {
    if goal == PC_4 {
        // The placement must fill exactly the 4 empty cells.
        let need = Board(PC_4.0 & !board.0);
        moves::<false>(board, piece).iter().any(|m| m.mask() == need)
    } else {
        moves::<false>(board, piece).iter().any(|m| {
            let mut n = board;
            n |= m.mask();
            n.clearshift() == goal
        })
    }
}