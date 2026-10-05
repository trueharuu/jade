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
        moves(board, piece).iter().any(|m| m.mask() == need)
    } else {
        moves(board, piece).iter().any(|m| {
            let mut n = board;
            n |= m.mask();
            n.clearshift() == goal
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The count bound for every reachable cell count, with `two_l` both ways.
    // This pins down the `cells == 20` boundary: at 20 filled cells the
    // two-line goal is already met, so both settings must ask for 5 pieces to
    // reach the full goal, not 0.
    #[test]
    fn count_bound_matches_goal() {
        for cells in (0..=40).step_by(4) {
            let four = pieces_to_goal(cells, false);
            assert_eq!(four, ((40 - cells) / 4) as usize, "cells {cells}");

            let two = pieces_to_goal(cells, true);
            if cells < 20 {
                assert_eq!(two, ((20 - cells) / 4) as usize, "cells {cells}, two_l");
            } else {
                assert_eq!(two, four, "cells {cells}, two_l");
            }
        }
    }

    #[test]
    fn goal_boards_are_full_rows() {
        assert_eq!(PC_4.0.count_ones(), 40);
        assert_eq!(PC_2.0.count_ones(), 20);
        assert_eq!(PC_4.0 & PC_2.0, PC_2.0);
    }

    #[test]
    fn playfield_bound() {
        assert!(!outside_playfield(PC_4));
        assert!(!outside_playfield(PC_2));
        assert!(!outside_playfield(Board::empty()));
        assert!(outside_playfield(Board(1 << 40)));
        assert!(outside_playfield(Board(1 << 59)));
    }

    #[test]
    fn a_full_goal_board_is_not_pruned() {
        // Both goals fill every cell of their rows, so no column is partly
        // empty and every seam region has a cell count that is a multiple of
        // four. Neither goal is pruned, so the goal test cannot be skipped
        // because of `pruned`. This contradicts the claim in
        // `docs/boolean-solver.md`; that file is out of date.
        assert!(!pruned(PC_4));
        assert!(!pruned(PC_2));
        assert!(!pruned(Board::empty()));
    }

    #[test]
    fn a_sealed_seam_with_a_bad_left_count_is_pruned() {
        // Column 1 filled in all four play rows seals the seam at column 0, so
        // no empty cell left of it can be reached from the right.
        let seam = jade_core::header::col_mask(1) & Board::lines(4).0;

        // Column 0 empty: its 4 cells are a multiple of four, so this is fine.
        assert!(!pruned(Board(seam)));

        // One filled cell in column 0: 1 is not a multiple of four.
        assert!(pruned(Board(seam | 1)));
    }
}
