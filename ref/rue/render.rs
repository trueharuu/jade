//! Rendering utilities.

use crate::board::Board;
use crate::header::WIDTH;
use crate::piece::Piece;
use crate::placement::Move;

/// Vertical bar.
pub const BAR: &str = "│";
/// Top-left corner.
pub const TOP_LEFT: &str = "┌";
/// Top-right corner.
pub const TOP_RIGHT: &str = "┐";
/// Bottom-left corner.
pub const BOTTOM_LEFT: &str = "└";
/// Bottom-right corner.
pub const BOTTOM_RIGHT: &str = "┘";
/// A single filled cell.
pub const CELL: &str = "\x1b[48;2;127;127;127m  \x1b[0m";
/// A single empty cell.
pub const EMPTY: &str = "  ";

/// Renders the cell for a given [`Piece`] color.
#[inline]
#[must_use]
pub fn cell(piece: Piece, i: &str) -> String {
    match piece {
        Piece::T => format!("\x1b[48;2;255;127;255m{i}\x1b[0m"),
        Piece::I => format!("\x1b[48;2;127;255;255m{i}\x1b[0m"),
        Piece::J => format!("\x1b[48;2;127;127;255m{i}\x1b[0m"),
        Piece::L => format!("\x1b[48;2;255;191;127m{i}\x1b[0m"),
        Piece::O => format!("\x1b[48;2;255;255;127m{i}\x1b[0m"),
        Piece::S => format!("\x1b[48;2;127;255;127m{i}\x1b[0m"),
        Piece::Z => format!("\x1b[48;2;255;127;127m{i}\x1b[0m"),
    }
}

/// Renders a single board.
#[inline]
#[must_use]
pub fn board(board: &Board) -> String {
    let mut s = String::new();
    s.push_str(TOP_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(TOP_RIGHT);
    s.push('\n');
    for y in (0..(board.height() + 2).max(6)).rev() {
        s.push_str(BAR);
        for x in 0..WIDTH {
            s.push_str(if board.get(x, y) { CELL } else { EMPTY });
        }
        s.push_str(BAR);
        s.push('\n');
    }

    s.push_str(BOTTOM_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(BOTTOM_RIGHT);
    s.push('\n');

    s
}

/// Renders two boards, showing the overlap between them.
#[inline]
#[must_use]
pub fn merge(red: &Board, blue: &Board) -> String {
    let mut s = String::new();
    s.push_str(TOP_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(TOP_RIGHT);
    s.push('\n');
    let height = red.height().max(blue.height());
    for y in (0..(height + 4).max(6)).rev() {
        s.push_str(BAR);
        for x in 0..WIDTH {
            s.push_str(&if red.get(x, y) && blue.get(x, y) {
                cell(Piece::T, "  ")
            } else if red.get(x, y) {
                cell(Piece::Z, "  ")
            } else if blue.get(x, y) {
                cell(Piece::I, "  ")
            } else {
                EMPTY.to_string()
            });
        }
        s.push_str(BAR);
        s.push('\n');
    }

    s.push_str(BOTTOM_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(BOTTOM_RIGHT);
    s.push('\n');

    s
}

/// Renders a board with a placement applied.
#[inline]
#[must_use]
pub fn placement<const N: usize>(board: &Board<N>, mv: &Move) -> String {
    let mut s = String::new();
    s.push_str(TOP_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(TOP_RIGHT);
    s.push('\n');
    let cells = mv.cells();
    for y in (0..(board.height() + 4).max(6)).rev() {
        s.push_str(BAR);
        for x in 0..WIDTH {
            s.push_str(&if cells.contains(&(x, y)) {
                if board.get(x, y) {
                    cell(mv.piece(), "--")
                } else {
                    cell(mv.piece(), "  ")
                }
            } else if board.get(x, y) {
                CELL.to_string()
            } else {
                EMPTY.to_string()
            });
        }
        s.push_str(BAR);
        s.push('\n');
    }

    s.push_str(BOTTOM_LEFT);
    s.push_str(&"─".repeat(WIDTH as usize * 2));
    s.push_str(BOTTOM_RIGHT);
    s.push('\n');

    s
}
