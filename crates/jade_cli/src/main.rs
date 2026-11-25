use std::collections::BTreeSet;
use std::fmt::Display;

use jade_oqb::bag::Bag;
use rayon::prelude::*;
use std::ops::Deref;
use std::str::FromStr;

use clap::Parser;

use fumen::Fumen;
use itertools::Itertools;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::queue::Queue;
use jade_core::render::BAR;
use jade_core::render::BOTTOM_LEFT;
use jade_core::render::BOTTOM_RIGHT;
use jade_core::render::CELL;
use jade_core::render::EMPTY;
use jade_core::render::LINE;
use jade_core::render::TOP_LEFT;
use jade_core::render::TOP_RIGHT;
use jade_core::render::cell;
use jade_core::rotation::Rotation;
use jade_pattern::Pattern;
use jade_pattern::disjoint::DisjointPattern;
use jade_solve::best::best;
use jade_solve::best::best_score;
use jade_solve::congruents::congruents;
use jade_solve::evaluate::Evaluator;
use jade_solve::fumen::encode_fumen;
use jade_solve::fumen::join;
use jade_solve::fumen::unglue;
use jade_solve::percent::percent;
use jade_solve::saves::Saves;
use jade_solve::setup::setups;
use jade_solve::solve;

#[derive(clap::Parser)]
pub struct Program {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    #[clap(subcommand)]
    Pattern(PatternSubcommand),

    /// Decide whether the queue can perform a perfect clear on the given field.
    Solve {
        #[arg(short, long)]
        pattern: Pattern,
        /// Starting field for the solve, as a fumen. Defaults to an empty
        /// field.
        #[arg(short, long)]
        field: Option<String>,
        /// Count a two-line (`PC_2`) field as success instead of the full
        /// field.
        #[arg(long = "2l")]
        two_l: bool,
        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    /// Returns the proportion of queues that can perform a perfect clear on the
    /// given field.
    Percent {
        #[arg(short, long)]
        pattern: Pattern,
        /// Starting field for the solve, as a fumen. Defaults to an empty
        /// field.
        #[arg(short, long)]
        field: Option<String>,
        /// Count a two-line (`PC_2`) field as success instead of the full
        /// field.
        #[arg(long = "2l")]
        two_l: bool,
        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    /// Finds all setups of `n` pieces that have some probability (or higher) of
    /// performing a perfect clear with the remaining pieces.
    Setup {
        #[arg(short, long)]
        pattern: DisjointPattern,

        /// The amount of pieces to place from `pattern` for the setup.
        #[arg(short, long)]
        n: usize,

        /// The minimum probability of a perfect clear for a setup to be
        /// reported. If no cutoff is specified, the best setup is reported
        /// instead.
        #[arg(short, long)]
        cutoff: Option<f64>,

        /// Report at most this many setups. Requires auto mode, so no
        /// `--cutoff`. Rows of the same rate keep the order they were found
        /// in.
        #[arg(short, long)]
        limit: Option<usize>,

        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,

        /// Report all congruents that can be built using the used pieces.
        #[arg(long)]
        build: bool,
    },

    /// Reports every move sequence that builds the given field.
    Congruents {
        #[arg(short, long)]
        pattern: Pattern,

        /// The field to build.
        #[arg(short, long)]
        field: String,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    Saves {
        #[arg(short, long)]
        pattern: Pattern,

        /// Starting field for the solve.
        #[arg(short, long)]
        field: Option<String>,

        /// The save criteria to consider.
        #[arg(short, long, default_value_t = Saves::all())]
        save: Saves,
    },

    /// Finds the best scoring paths for a set of queues
    Score {
        #[arg(short, long)]
        pattern: Pattern,

        /// Starting field for the solve as a fumen.
        #[arg(short, long)]
        field: Option<String>,

        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    /// Finds the single best scoring path for a single queue
    Scoring {
        #[arg(short, long)]
        pattern: Queue,

        /// Starting field for the solve as a fumen.
        #[arg(short, long)]
        field: Option<String>,

        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    Tree {
        #[arg(short, long)]
        pattern: Queue,
        #[arg(short, long)]
        bag: Bag,
        /// Starting field.
        #[arg(short, long)]
        field: Option<String>,
        /// Count a two-line field as success.
        #[arg(long = "2l")]
        two_l: bool,
        /// Number of threads.
        #[arg(short, long, default_value_t = 1)]
        threads: usize,
    },

    Render {
        fields: Vec<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum PatternSubcommand {
    Expand {
        pattern: Pattern,
        #[arg(short, long, default_value_t = String::from("\n"))]
        separator: String,
    },

    Count {
        pattern: Pattern,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold(bool);

impl FromStr for Hold {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "use" => Ok(Hold(true)),
            "avoid" => Ok(Hold(false)),
            _ => Err(format!("invalid hold value: {s}")),
        }
    }
}

impl Display for Hold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 {
            write!(f, "use")
        } else {
            write!(f, "avoid")
        }
    }
}

impl Deref for Hold {
    type Target = bool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[must_use]
pub fn letterize(x: usize) -> String {
    // returns `x` as a base-26 string with 'A' as 0, 'B' as 1, ..., 'Z' as 25,
    // 'AA' as 26, etc.
    let mut x = x;
    let mut s = String::new();
    loop {
        let c = (x % 26) as u8 + b'A';
        s.push(c as char);
        x /= 26;
        if x == 0 {
            break;
        }
    }

    s.chars().rev().collect()
}

pub fn main() {
    let program = Program::parse();

    match program.cmd {
        Command::Render { fields } => {
            for (i, field) in fields.iter().enumerate() {
                let f = Fumen::decode(field).unwrap();
                for (j, page) in f.pages.iter().enumerate() {
                    println!("{}{}:", i + 1, letterize(j));

                    println!("{TOP_LEFT}{}{TOP_RIGHT}", LINE.repeat(10));
                    for y in (0..6).rev() {
                        print!("{BAR}");
                        for x in page.field[y] {
                            print!(
                                "{}",
                                match x {
                                    fumen::CellColor::Empty => EMPTY.to_string(),
                                    fumen::CellColor::I => cell(Piece::I, EMPTY),
                                    fumen::CellColor::O => cell(Piece::O, EMPTY),
                                    fumen::CellColor::T => cell(Piece::T, EMPTY),
                                    fumen::CellColor::L => cell(Piece::L, EMPTY),
                                    fumen::CellColor::J => cell(Piece::J, EMPTY),
                                    fumen::CellColor::S => cell(Piece::S, EMPTY),
                                    fumen::CellColor::Z => cell(Piece::Z, EMPTY),
                                    fumen::CellColor::Grey => CELL.to_string(),
                                }
                            );
                        }
                        println!("{BAR}");
                    }
                    println!("{BOTTOM_LEFT}{}{BOTTOM_RIGHT}", LINE.repeat(10));
                }
            }
        }

        Command::Pattern(pattern_cmd) => match pattern_cmd {
            PatternSubcommand::Expand { pattern, separator } => {
                let expanded = pattern.expand();

                println!(
                    "{}",
                    expanded.iter().map(ToString::to_string).join(&separator)
                );
            }

            PatternSubcommand::Count { pattern } => {
                let expanded = pattern.expand();
                println!("{}", expanded.len());
            }
        },

        Command::Solve {
            pattern,
            field,
            two_l,
            save,
            hold,
        } => {
            let board = parse_board(field.as_deref());

            let queues = pattern.expand();

            if queues.len() > 1 {
                eprintln!("error: pattern must expand to a single queue");
                std::process::exit(2);
            }

            let queue = &queues.first().unwrap();
            if solve::reachable(board, queue, two_l, save, *hold) {
                std::process::exit(0);
            }
            std::process::exit(1);
        }

        Command::Percent {
            pattern,
            field,
            two_l,
            save,
            hold,
        } => {
            let board = parse_board(field.as_deref());
            let queues = pattern.expand();
            let (n, d) = percent(board, queues, two_l, save, *hold);
            let p = n as f64 / d as f64;
            let mp = minimum_precision(d) - 2;
            println!("{n}/{d} ({:.mp$}%)", p * 100.0);
        }

        Command::Setup {
            pattern,
            n,
            cutoff,
            limit,
            hold,
            save,
            build,
        } => {
            if limit.is_some() && cutoff.is_some() {
                eprintln!("error: --limit cannot be used with --cutoff");
                std::process::exit(2);
            }

            let ss = setups(&pattern.clone(), n, *hold);

            eprintln!(
                "{} ({})",
                ss.values().map(std::vec::Vec::len).sum::<usize>(),
                ss.keys().join(" ")
            );

            let pat = pattern.join().expand();

            // A cutoff of 1.0 means "only setups that always clear", which is
            // the threshold `percent` applied per board. Below 1.0 the setup
            // just has to reach that rate. With no cutoff, the evaluator
            // tracks the best rate instead.
            // Without a cutoff the evaluator keeps the best rates it has seen,
            // so a limit tells it how many rates to hold.
            let evaluator = Evaluator::with_limit(cutoff, limit.unwrap_or(1));

            // Results are printed at the end. Without a cutoff the best rate
            // can grow during the run, so `keep` decides what to report.
            let mut results: Vec<(Queue, Board, u64, u64)> = Vec::new();

            for (queue, boards) in ss {
                // The remaining queues after the setup consumed its pieces.
                // The dedup has to happen after `consume`: it is not
                // injective, so distinct queues can consume to the same
                // queue and `run_group` expects distinct solves.
                let solves: Vec<Vec<Piece>> = pat
                    .iter()
                    .map(|x| x.consume(&queue))
                    .collect::<BTreeSet<_>>()
                    .iter()
                    .map(|x| x.as_slice().to_vec())
                    .collect();

                let d = solves.len() as u64;

                // The evaluator parallelises over the boards itself.
                for (board, n) in evaluator.run_group(&solves, &boards, save, *hold) {
                    results.push((queue, board, n, d));
                }
            }

            results.retain(|&(_, _, n, d)| evaluator.keep(n, d));

            // The sort is stable, so rows of equal rate keep the order the
            // search found them in.
            results.sort_by(|a, b| rate(b.2, b.3).total_cmp(&rate(a.2, a.3)));
            if let Some(limit) = limit {
                results.truncate(limit);
            }

            for (queue, board, n, d) in &results {
                let p = rate(*n, *d);
                let mp = minimum_precision(*d);
                let left = if build {
                    let congruent_fumens: Vec<String> = congruents(*board, &[*queue], *hold)
                        .iter()
                        .map(|x| {
                            let mut b = Board::empty();
                            let mut f = Fumen::default();
                            let mut lc = 0;
                            for &m in x {
                                let page = f.add_page();
                                page.piece = Some(fumen::Piece {
                                    kind: match m.piece() {
                                        Piece::I => fumen::PieceType::I,
                                        Piece::O => fumen::PieceType::O,
                                        Piece::T => fumen::PieceType::T,
                                        Piece::L => fumen::PieceType::L,
                                        Piece::J => fumen::PieceType::J,
                                        Piece::S => fumen::PieceType::S,
                                        Piece::Z => fumen::PieceType::Z,
                                    },
                                    rotation: match m.rotation() {
                                        Rotation::North => fumen::RotationState::North,
                                        Rotation::East => fumen::RotationState::East,
                                        Rotation::South => fumen::RotationState::South,
                                        Rotation::West => fumen::RotationState::West,
                                    },
                                    x: m.x() as u32,
                                    y: m.y() as u32 - lc as u32,
                                });
                                b |= m.mask();
                                b = b.clearshift();
                                lc = b.line_clears();
                            }
                            unglue(&f.encode()).unwrap()
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    let refs: Vec<&str> = congruent_fumens.iter().map(String::as_str).collect();
                    join(&refs)
                } else {
                    encode_fumen(board)
                };
                println!("{left}\t{n}\t{d}\t{p:.mp$}\t{queue}");
            }
        }

        Command::Congruents {
            pattern,
            field,
            hold,
        } => {
            let target = parse_board(Some(&field));
            let queues: Vec<Queue> = pattern.expand().into_iter().collect();

            congruents(target, &queues, *hold)
                .iter()
                .map(|x| {
                    let mut b = Board::empty();
                    let mut f = Fumen::default();
                    let mut lc = 0;
                    for &m in x {
                        let page = f.add_page();

                        page.piece = Some(fumen::Piece {
                            kind: match m.piece() {
                                Piece::I => fumen::PieceType::I,
                                Piece::O => fumen::PieceType::O,
                                Piece::T => fumen::PieceType::T,
                                Piece::L => fumen::PieceType::L,
                                Piece::J => fumen::PieceType::J,
                                Piece::S => fumen::PieceType::S,
                                Piece::Z => fumen::PieceType::Z,
                            },
                            rotation: match m.rotation() {
                                Rotation::North => fumen::RotationState::North,
                                Rotation::East => fumen::RotationState::East,
                                Rotation::South => fumen::RotationState::South,
                                Rotation::West => fumen::RotationState::West,
                            },
                            x: m.x() as u32,
                            y: m.y() as u32 - lc as u32,
                        });

                        b |= m.mask();
                        b = b.clearshift();
                        lc = b.line_clears();
                    }

                    f.encode()
                })
                .map(|x| unglue(&x).unwrap().clone())
                .collect::<BTreeSet<_>>()
                .iter()
                .for_each(|x| println!("{x}"));
        }

        Command::Saves {
            pattern,
            field,
            save,
        } => {
            let board = parse_board(field.as_deref());

            for piece in Piece::ALL {
                if !save.has(piece) {
                    continue;
                }

                // run percent with this specific piece as save
                let queues = pattern.expand();
                let mut save = Saves::empty();
                save.add(piece);
                let (n, d) = percent(board, queues, false, save, true);
                let p = n as f64 / d as f64;
                let mp = minimum_precision(d) - 2;
                println!("{piece}: {n}/{d} ({:.mp$}%)", p * 100.0);
            }
        }

        Command::Score {
            pattern,
            field,
            save,
            hold,
        } => {
            let board = parse_board(field.as_deref());

            let queues = pattern.expand();
            let total = queues.len();

            let results: Vec<_> = queues
                .par_iter()
                .map(|queue| best_score(board, queue.as_slice(), false, save, *hold))
                .collect();

            let mut solved = 0usize;
            let mut score_sum: u64 = 0;
            let mut groups: std::collections::BTreeMap<(String, u32), usize> =
                std::collections::BTreeMap::new();

            for res in results {
                match res {
                    Some(r) => {
                        solved += 1;
                        score_sum += u64::from(r.score);
                        let events = r
                            .events
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ");
                        *groups.entry((events, r.score)).or_insert(0) += 1;
                    }
                    None => {
                        *groups.entry((String::new(), 0)).or_insert(0) += 1;
                    }
                }
            }

            let mut rows: Vec<((String, u32), usize)> = groups.into_iter().collect();
            rows.sort_by(|a, b| {
                b.0.1
                    .cmp(&a.0.1)
                    .then(b.1.cmp(&a.1))
                    .then(a.0.0.cmp(&b.0.0))
            });

            for ((events, score), count) in rows {
                println!("{count}\t{score}\t{events}");
            }

            let average = if total == 0 {
                0.0
            } else {
                score_sum as f64 / total as f64
            };
            eprintln!("{average} ({solved}/{total})");
        }

        Command::Scoring {
            pattern: queue,
            field,
            save,
            hold,
        } => {
            let board = parse_board(field.as_deref());

            if let Some(res) = best(board, queue.as_slice(), false, save, *hold) {
                let mut b = board;
                let mut f = Fumen::default();
                let mut cleared = 0i32;
                for (i, &m) in res.moves.iter().enumerate() {
                    let page = f.add_page();
                    if i == 0 {
                        for y in 0..4 {
                            for x in 0..10 {
                                if board.get(x as i32, y as i32) {
                                    page.field[y][x] = fumen::CellColor::Grey;
                                }
                            }
                        }
                    }
                    page.piece = Some(fumen::Piece {
                        kind: match m.piece() {
                            Piece::I => fumen::PieceType::I,
                            Piece::O => fumen::PieceType::O,
                            Piece::T => fumen::PieceType::T,
                            Piece::L => fumen::PieceType::L,
                            Piece::J => fumen::PieceType::J,
                            Piece::S => fumen::PieceType::S,
                            Piece::Z => fumen::PieceType::Z,
                        },
                        rotation: match m.rotation() {
                            Rotation::North => fumen::RotationState::North,
                            Rotation::East => fumen::RotationState::East,
                            Rotation::South => fumen::RotationState::South,
                            Rotation::West => fumen::RotationState::West,
                        },
                        x: m.x() as u32,
                        y: (m.y() - cleared) as u32,
                    });
                    let lines_this = (b | m.mask()).line_clears().saturating_sub(b.line_clears());
                    // apply placement for b tracking
                    b |= m.mask();
                    b = b.clearshift();
                    cleared += lines_this;
                }
                let fumen_str = unglue(&f.encode()).unwrap_or_else(|_| f.encode());

                println!("{}", res.score);
                println!("{fumen_str} {}", f.encode());
                println!("{}", res.events.iter().map(ToString::to_string).join(", "));
            } else {
                eprintln!("error: no solution");
                std::process::exit(1);
            }
        }

        Command::Tree { pattern: queue, bag, field, two_l, threads } => {
            let field = parse_board(field.as_deref());
            let mut queue = queue;
            let hold = if queue.len() > 0 {
                let h = queue.as_slice()[0];
                queue.pop_front();
                Some(h)
            } else {
                None
            };
            let mut tree = jade_oqb::oqb(field, queue, bag, two_l, threads, hold);
            jade_oqb::fill_moves(&mut tree, two_l, jade_solve::saves::Saves::empty(), true);
            println!("{}", tree.to_indented_string(field));
        }
    }
}

/// Decodes the `--field` argument, or returns an empty field when it is absent.
/// Exits the process if the fumen cannot be decoded.
fn parse_board(field: Option<&str>) -> Board {
    match field {
        Some(s) => match jade_solve::parse_fumen(s) {
            Ok(board) => board,
            Err(msg) => {
                eprintln!("error: failed to parse fumen: {msg}");
                std::process::exit(2);
            }
        },
        None => Board::empty(),
    }
}

/// Returns the success rate of `n` solves out of `d`.
fn rate(n: u64, d: u64) -> f64 {
    n as f64 / d as f64
}

/// Returns the minimum precision such that no two values with denominator
/// `denom` map to the same value.
const fn minimum_precision(denom: u64) -> usize {
    denom.ilog10() as usize + 1
}
