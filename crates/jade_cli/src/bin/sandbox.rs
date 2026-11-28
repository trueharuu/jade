//! Sandbox.

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::render;
use jade_nav::fast;

/// Main.
pub fn main() {
    let mut b = Board::empty();
    b.set(8, 0);

    let mvs = fast::generate::<{ Piece::T }, true>(&b);

    for mv in &mvs {
        println!("{}", render::placement(&b, &mv));
        println!("{mv}");
    }
}
