use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::render;
use jade_nav::op;
use jade_nav::oracle;

fn main() {
    let mut b = Board::empty();
    b.set(0, 0);
    b.set(0, 1);
    b.set(1, 1);
    b.set(1, 2);

    b.set(0, 2);
    b.set(0, 3);
    b.set(1, 3);
    b.set(2, 3);

    b.set(4, 0);
    b.set(4, 1);
    b.set(5, 0);
    b.set(5, 1);

    b.set(6, 1);
    b.set(7, 1);
    b.set(8, 0);
    b.set(8, 1);

    b.set(9, 0);
    b.set(9, 1);
    b.set(9, 2);
    b.set(9, 3);

    let us = op::usable_map::<{ Piece::S }>(&b);
    println!("{}", render::merge(&b, &us[0]));
    println!("{}", render::merge(&b, &us[1]));
    let buffer = !Board::lines(4);
    let mut n = 0;
    for s in &oracle::generate::<{ Piece::S }>(&b, &Board::lines(4)) {
        assert_eq!(s.mask() & buffer, Board::empty(), "placement spills into buffer rows");
        println!("{}", render::placement(&b, &s));
        n += 1;
    }
    println!("field placements: {n}");
}
