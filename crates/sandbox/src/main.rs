use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::render;
use jade_core::rotation::Rotation;

fn main() {
    let mut b = Board::empty();
    b.set(0, 0);
    b.set(0, 1);
    b.set(1, 1);
    b.set(1, 2);

    let mv = Move::new(Piece::T, Rotation::North, 2, 0);

    let msk = mv.mask();
    println!("{}", render::board(&b));
    println!("{}", render::board(&msk));
    println!("{}", render::placement(&b, &mv));
}
