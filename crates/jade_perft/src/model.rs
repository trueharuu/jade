use std::fmt;

use clap::ValueEnum;
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_nav::buffer::Moves;
use jade_nav::fast;
use jade_nav::oracle;

/// The move generator used by a perft run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Model {
    #[default]
    Oracle,
    Fast,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oracle => write!(f, "oracle"),
            Self::Fast => write!(f, "fast"),
        }
    }
}

/// Generates all reachable landed placements of `piece` on `board` for the
/// selected model.
#[inline]
#[must_use]
pub fn moves(model: Model, piece: Piece, board: Board) -> Moves {
    match model {
        Model::Oracle => oracle_moves(piece, board),
        Model::Fast => fast_moves(piece, board),
    }
}

/// Scalar BFS oracle, dispatched over the const-generic `Piece` parameter.
#[inline]
#[must_use]
fn oracle_moves(piece: Piece, board: Board) -> Moves {
    match piece {
        Piece::T => oracle::generate::<{ Piece::T }>(&board),
        Piece::I => oracle::generate::<{ Piece::I }>(&board),
        Piece::J => oracle::generate::<{ Piece::J }>(&board),
        Piece::L => oracle::generate::<{ Piece::L }>(&board),
        Piece::O => oracle::generate::<{ Piece::O }>(&board),
        Piece::S => oracle::generate::<{ Piece::S }>(&board),
        Piece::Z => oracle::generate::<{ Piece::Z }>(&board),
    }
}

/// Bitboard-accelerated fast move generator.
#[inline]
#[must_use]
fn fast_moves(piece: Piece, board: Board) -> Moves {
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

/// Builds a queue from a piece-string like `TZSIJL`. Only used by tests to
/// construct fixed queues; the binary takes queue patterns instead.
#[cfg(test)]
#[must_use]
pub fn parse_queue(src: &str) -> Vec<Piece> {
    src.chars()
        .map(|c| match c {
            'T' => Piece::T,
            'I' => Piece::I,
            'J' => Piece::J,
            'L' => Piece::L,
            'O' => Piece::O,
            'S' => Piece::S,
            'Z' => Piece::Z,
            other => panic!("invalid piece char: {other}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_empty_board() {
        for &piece in &Piece::ALL {
            let n = moves(Model::Oracle, piece, Board::empty()).iter().count();
            assert!(n > 0, "{piece}: no placements from an empty field");
        }
    }
}
