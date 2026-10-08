use std::{collections::BTreeSet, str::FromStr};

use jade_core::{piece::Piece, queue::Queue};
use jade_pattern::Pattern;

fn main() {
    let f = Pattern::from_str("[TSZ]!*!").unwrap();
    let used = Queue::from_slice(&[Piece::T, Piece::Z]);

    println!("{}", f.expand().len());
    println!("{}", f.expand().iter().map(|q| q.consume(&used.as_slice())).collect::<BTreeSet<_>>().len());

    // for q in f.expand() {
    //     println!("{q} - {used} -> {}", q.consume(&used.as_slice()));
    // }
}
