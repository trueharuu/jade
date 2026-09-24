use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::render;
use jade_nav::fast;
use jade_nav::op;

fn main() {
    const P: Piece = Piece::S;

    let mut b = Board::empty();
    b.set(3, 0);
    b.set(4, 0);
    b.set(5, 0);
    b.set(6, 0);
    b.set(4, 1);
    b.set(5, 1);
    b.set(5, 2);
    b.set(6, 2);

    let us = op::usable_map::<P>(&b);
    println!("{}", render::merge(&b, &us[0]));
    println!("{}", render::merge(&b, &us[1]));
    let mut n = 0;
    for s in &fast::generate::<P>(&b) {
        let result = (b | s.mask()).clearshift();
        n += 1;
        let bad = result.has_imbalanced_split() || result.has_isolated_cell();
        println!("{}\n{n}. {s:?} {bad}", render::placement(&b, &s));
            
    }
}
