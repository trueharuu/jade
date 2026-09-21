#![feature(min_adt_const_params)]
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::rotation::Rotation;
use jade_nav::buffer::Moves;
use jade_nav::fast;
use jade_nav::oracle;

fn counts(m: &Moves) -> [u32; Rotation::NB] {
    let mut out = [0; Rotation::NB];
    for (i, b) in m.mask.iter().enumerate() {
        out[i] = b.popcount();
    }
    out
}

fn run(name: &str, board: Board) {
    for &p in &Piece::ALL {
        let o = match p {
            Piece::T => oracle::generate::<{ Piece::T }>(&board),
            Piece::I => oracle::generate::<{ Piece::I }>(&board),
            Piece::J => oracle::generate::<{ Piece::J }>(&board),
            Piece::L => oracle::generate::<{ Piece::L }>(&board),
            Piece::O => oracle::generate::<{ Piece::O }>(&board),
            Piece::S => oracle::generate::<{ Piece::S }>(&board),
            Piece::Z => oracle::generate::<{ Piece::Z }>(&board),
        };
        let f = match p {
            Piece::T => fast::generate::<{ Piece::T }>(&board),
            Piece::I => fast::generate::<{ Piece::I }>(&board),
            Piece::J => fast::generate::<{ Piece::J }>(&board),
            Piece::L => fast::generate::<{ Piece::L }>(&board),
            Piece::O => fast::generate::<{ Piece::O }>(&board),
            Piece::S => fast::generate::<{ Piece::S }>(&board),
            Piece::Z => fast::generate::<{ Piece::Z }>(&board),
        };
        let co = counts(&o);
        let cf = counts(&f);
        let so: u32 = co.iter().sum();
        let sf: u32 = cf.iter().sum();
        let mark = if so == sf && co == cf { "  ok" } else { "DIFF" };
        println!(
            "{name:>4} {p}: oracle={co:?}({so:3})  fast={cf:?}({sf:3})  {mark}"
        );
    }
}

fn stack(y: i32, cols: std::ops::Range<i32>) -> Board {
    let mut b = 0u64;
    for yy in 0..y {
        for x in cols.clone() {
            b |= 1u64 << (yy * 10 + x) as u32;
        }
    }
    Board::new(b)
}

fn fmt(bb: &[jade_core::board::Board; 4]) -> String {
    bb.iter()
        .enumerate()
        .filter(|(_, t)| t.any())
        .map(|(i, t)| {
            let coords: Vec<String> = t.iter().map(|(x, y)| format!("({x},{y})")).collect();
            format!("[{i}]({})={}", t.popcount(), coords.join(" "))
        })
        .collect::<Vec<_>>()
        .join("  ")
}

const PIECES: [Piece; 7] = [
    Piece::T,
    Piece::I,
    Piece::J,
    Piece::L,
    Piece::O,
    Piece::S,
    Piece::Z,
];

fn both<const P: Piece>(b: &Board) -> (Moves, Moves) {
    (
        match P {
            Piece::T => oracle::generate::<{ Piece::T }>(b),
            Piece::I => oracle::generate::<{ Piece::I }>(b),
            Piece::J => oracle::generate::<{ Piece::J }>(b),
            Piece::L => oracle::generate::<{ Piece::L }>(b),
            Piece::O => oracle::generate::<{ Piece::O }>(b),
            Piece::S => oracle::generate::<{ Piece::S }>(b),
            Piece::Z => oracle::generate::<{ Piece::Z }>(b),
        },
        match P {
            Piece::T => fast::generate::<{ Piece::T }>(b),
            Piece::I => fast::generate::<{ Piece::I }>(b),
            Piece::J => fast::generate::<{ Piece::J }>(b),
            Piece::L => fast::generate::<{ Piece::L }>(b),
            Piece::O => fast::generate::<{ Piece::O }>(b),
            Piece::S => fast::generate::<{ Piece::S }>(b),
            Piece::Z => fast::generate::<{ Piece::Z }>(b),
        },
    )
}

fn main() {
    let empty = Board::empty();
    let row0 = Board::new(0x78);
    let wall = Board::new((1u64 << 10) | 1);
    let row2 = stack(3, 0..10); // three full bottom rows
    let well = stack(4, 0..6) | stack(4, 7..10); // well in column 6
    let bump = stack(1, 2..8); // a row-1 ledge over cols 2..7

    for (name, b) in [
        ("empty", empty),
        ("row0", row0),
        ("wall", wall),
        ("row2", row2),
        ("well", well),
        ("bump", bump),
    ] {
        println!("=== {name} ===");
        for &p in &Piece::ALL {
            let (o, f) = match p {
                Piece::T => both::<{ Piece::T }>(&b),
                Piece::I => both::<{ Piece::I }>(&b),
                Piece::J => both::<{ Piece::J }>(&b),
                Piece::L => both::<{ Piece::L }>(&b),
                Piece::O => both::<{ Piece::O }>(&b),
                Piece::S => both::<{ Piece::S }>(&b),
                Piece::Z => both::<{ Piece::Z }>(&b),
            };
            let co = counts(&o);
            let cf = counts(&f);
            let so: u32 = co.iter().sum();
            let sf: u32 = cf.iter().sum();
            let mark = if so == sf && co == cf { "ok" } else { "DIFF" };
            println!("{name} {p}: oracle={co:?}({so:3})  fast={cf:?}({sf:3})  {mark}");
        }
    }

    let mut bad = 0u32;
    let mut ran = 0u32;
    let mut state = 0x9E3779B97F4A7C15u64;
    for trial in 0..20000 {
        if trial % 2000 == 0 {
            eprintln!("trial {trial}/20000");
        }
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let p = Piece::ALL[(trial % 7) as usize];
        let mut raw = 0u64;
        let mut s = state;
        for _ in 0..60 {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            if s & 0x7f37 < 0x2f00 {
                raw |= 1u64 << (s % 60);
            }
        }
        let b = Board::new(raw & jade_core::header::MASK);
        let (o, f) = match p {
            Piece::T => both::<{ Piece::T }>(&b),
            Piece::I => both::<{ Piece::I }>(&b),
            Piece::J => both::<{ Piece::J }>(&b),
            Piece::L => both::<{ Piece::L }>(&b),
            Piece::O => both::<{ Piece::O }>(&b),
            Piece::S => both::<{ Piece::S }>(&b),
            Piece::Z => both::<{ Piece::Z }>(&b),
        };
        ran += 1;
        if o.mask != f.mask {
            bad += 1;
            if bad <= 5 {
                println!("MISMATCH trial={trial} {p} raw={raw:#x}");
                println!("  oracle={:?}", fmt(&o.mask));
                println!("  fast  ={:?}", fmt(&f.mask));
            }
        }
    }
    println!("fuzz: {ran} generated, {bad} mismatches");
}