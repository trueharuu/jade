use jade_core::{board::Plane, piece::Piece, placement::Move, render, rotation::Rotation};

fn main() {
    let mut b = Plane::<1>::empty();
    b.set(0, 0, 0);
    b.set(0, 0, 1);
    b.set(0, 1, 1);
    b.set(0, 1, 2);

    let mv = Move::new(Piece::T, Rotation::North, 2, 0);

    let msk = mv.mask().unwrap();
    println!("{}", render::board(&b));
    println!("{}", render::board(&Plane::<1>::from_array([msk])));
}
