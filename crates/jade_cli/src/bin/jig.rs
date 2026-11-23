use fumen::CellColor;
use fumen::Fumen;
use jade_core::piece::Piece;
use jade_pattern::Pattern;
use jade_solve::Saves;
use jade_solve::parse_fumen;

pub fn main() {
    let base = [
        (
            "jigsaw",
            "v115@9gilwwEewhglAtxwEewhBtR4Rpi0whAtR4wwRpBeg0?whJeAgH",
        ),
        (
            "jigsaw-m",
            "v115@9gwhEewwi0whEexwQ4g0whilRpBtR4whglBeRpwwBt?Q4JeAgH",
        ),
        (
            "pcosaw",
            "v115@9gBthlEewhwwBtglEewhxwR4Rpi0whwwR4glRpBeg0?whJeAgH",
        ),
        (
            "pcosaw-m",
            "v115@9gwhEeh0R4whEeg0R4wwwhilRpBtxwwhglBeRpg0Bt?wwJeAgH",
        ),
        (
            "altjaw",
            "v115@9gzhwwEeh0AtxwEeg0BtR4ilRpg0AtR4wwglBeRpJe?AgH",
        ),
        (
            "altjaw-m",
            "v115@ChwwzhEexwQ4hlRpi0BtR4glRpBeg0wwBtQ4glJeAg?H",
        ),
    ];

    for i in 0..=4 {
        for missing in Pattern::parse(&format!("*c{i}")).unwrap().expand() {
            for (name, setup) in base {
                let mut f = Fumen::decode(setup).unwrap();
                for row in &mut f.pages[0].field {
                    for col in row {
                        let p = match col {
                            CellColor::T => Piece::T,
                            CellColor::I => Piece::I,
                            CellColor::O => Piece::O,
                            CellColor::L => Piece::L,
                            CellColor::J => Piece::J,
                            CellColor::S => Piece::S,
                            CellColor::Z => Piece::Z,
                            _ => continue,
                        };

                        if missing.contains(p) {
                            *col = CellColor::Empty;
                        }
                    }
                }

                let u = f.encode();

                let sol = Pattern::parse(&if missing.is_empty() {
                    "*p4".into()
                } else {
                    format!(
                        "[{missing}]!*p{}",
                        if missing.len() == 4 {
                            3
                        } else {
                            11 - (7 - missing.len())
                        }
                    )
                })
                .unwrap();

                let b = parse_fumen(&u).unwrap();

                let (n, d) =
                    jade_solve::percent::percent(b, sol.expand(), false, Saves::empty(), true);

                println!("{u}\t{missing}\t{name}\t=FUMEN(\"{u}\")\t=TRUE\t={n}/{d}");
            }
        }
    }
    // let txt = std::fs::read_to_string("input").unwrap();
    // for line in txt.lines() {
    //     let mut args = line.split_ascii_whitespace();
    //     let fu = args.next().unwrap();
    //     let b = parse_fumen(fu).unwrap();
    //     let missing = args.next().unwrap_or_default();

    //     // get percent and print it
    //     let (n, d) = jade_solve::percent::percent(b, sol.expand(), false,
    // Saves::empty(), true);     println!("={n}/{d}");
    // }
}
