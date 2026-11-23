use jade_core::{board::Board, piece::Piece, render};
use jade_nav::fast;

pub fn main() {
    let mut b = Board::empty();
    b.set(8, 0);

    let mvs = fast::generate::<{ Piece::T }>(&b);

    for mv in &mvs {
        println!("{}", render::placement(&b, &mv));
        println!("{mv}");
    }
}