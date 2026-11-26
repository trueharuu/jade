//! Dispatch from a runtime `Piece` to the const-generic move generator.

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_nav::buffer::Moves;
use jade_nav::fast;

/// Returns every legal placement of `piece` on `board`.
///
/// The generator is generic over a const `Piece` so that it can unroll the
/// rotation planes at compile time. This function is the only place that
/// expands a runtime `Piece` into those calls.
#[must_use]
pub fn moves<const SPINS: bool>(board: Board, piece: Piece) -> Moves {
    match piece {
        Piece::T => fast::generate::<{ Piece::T }, SPINS>(&board),
        Piece::I => fast::generate::<{ Piece::I }, SPINS>(&board),
        Piece::J => fast::generate::<{ Piece::J }, SPINS>(&board),
        Piece::L => fast::generate::<{ Piece::L }, SPINS>(&board),
        Piece::O => fast::generate::<{ Piece::O }, SPINS>(&board),
        Piece::S => fast::generate::<{ Piece::S }, SPINS>(&board),
        Piece::Z => fast::generate::<{ Piece::Z }, SPINS>(&board),
    }
}
