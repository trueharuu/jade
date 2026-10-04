pub mod unglue;

use std::sync::Arc;
use std::sync::Mutex;

use clap::Parser;
use fumen::Fumen;
use itertools::Itertools;
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::render;
use jade_core::rotation::Rotation;
use jade_nav::oracle;
use jade_pattern::Pattern;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;

#[derive(clap::Parser)]
pub struct Program {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    #[clap(subcommand)]
    Pattern(PatternSubcommand),

    /// Decide whether the queue can reach a full board using hold.
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
        #[arg(short, long)]
        save: Option<Pattern>,
    },

    Move {
        #[arg(short, long, default_value_t = String::from("v115@vhAAgh"))]
        field: String,
        piece: Piece,
    },

    /// Report every placement sequence that builds a target field.
    Congruents {
        /// Pattern whose expansions are searched.
        #[arg(short, long)]
        pattern: Pattern,
        /// Target field, as a fumen. This is the field to build, not a start
        /// field, so the search always starts from an empty field.
        #[arg(short, long)]
        field: String,
    },

    /// Report the fraction of a pattern's queues that have a solve.
    Percent {
        /// Pattern whose expansions are solved.
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
        #[arg(short, long)]
        save: Option<Pattern>,
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

pub fn main() {
    let program = Program::parse();

    match program.cmd {
        Command::Pattern(pattern_cmd) => match pattern_cmd {
            PatternSubcommand::Expand { pattern, separator } => {
                let expanded = pattern.expand();

                println!(
                    "{}",
                    expanded.iter().map(|x| x.iter().join("")).join(&separator)
                );
            }

            PatternSubcommand::Count { pattern } => {
                let expanded = pattern.expand();
                println!("{}", expanded.len());
            }
        },

        Command::Move { field, piece } => {
            let board = match jade_solve::parse_fumen(&field) {
                Ok(board) => board,
                Err(msg) => {
                    eprintln!("error: failed to parse fumen: {msg}");
                    std::process::exit(1);
                }
            };

            let placements = match piece {
                Piece::T => oracle::generate::<{ Piece::T }>(&board),
                Piece::I => oracle::generate::<{ Piece::I }>(&board),
                Piece::J => oracle::generate::<{ Piece::J }>(&board),
                Piece::L => oracle::generate::<{ Piece::L }>(&board),
                Piece::O => oracle::generate::<{ Piece::O }>(&board),
                Piece::S => oracle::generate::<{ Piece::S }>(&board),
                Piece::Z => oracle::generate::<{ Piece::Z }>(&board),
            };

            for p in &placements {
                let nx = (board | p.mask()).clearshift();
                if nx.has_imbalanced_split() || nx.has_isolated_cell() {
                    continue;
                }
                println!("{}", render::placement(&board, &p));
            }
        }

        Command::Solve {
            pattern: queue,
            field,
            two_l,
            save,
        } => {
            let board = parse_board(field.as_deref());
            let save = parse_save(save.as_ref());

            // A field that can never be filled has no solution. Reject it
            // before the search, which would otherwise prune the root anyway.
            if jade_solve::is_unfillable(board) {
                // return failure exit code
                std::process::exit(1);
            }

            let yes = queue
                .expand()
                .iter()
                .any(|q| has_solve(board, q, two_l, save.as_deref()));

            if yes {
                // return success exit code
                std::process::exit(0);
            } else {
                // return failure exit code
                std::process::exit(1);
            }
        }

        Command::Percent {
            pattern,
            field,
            two_l,
            save,
        } => {
            let board = parse_board(field.as_deref());
            let save = parse_save(save.as_ref());
            let queues = pattern.expand();
            let total = queues.len();

            // A field that can never be filled has no solution for any queue,
            // so every queue fails. Report that without running the search.
            if jade_solve::is_unfillable(board) {
                println!("0/{total}");
                std::process::exit(0);
            }

            let solved = queues
                .iter()
                .filter(|q| has_solve(board, q, two_l, save.as_deref()))
                .count();

            println!("{solved}/{total}");
        }

        Command::Congruents { pattern, field } => {
            let target = parse_board(Some(&field));

            // Every placement adds exactly four cells, so a target whose cell
            // count is not a multiple of four has no placement sequence.
            let cells = target.popcount();
            if !cells.is_multiple_of(4) {
                eprintln!("error: target field has {cells} cells, which is not a multiple of 4");
                std::process::exit(2);
            }

            // One set for the whole command, not one per queue. Hold lets a
            // path use fewer queue pieces than the queue holds, so one sequence
            // can be reachable from more than one queue, and it prints once.
            let seen = Arc::new(Mutex::new(jade_solve::Paths::default()));

            pattern.expand().into_par_iter().for_each(|queue| {
                // println!("{queue:?}");
                let mut x = seen.lock().unwrap();
                let paths = jade_solve::congruents(target, &queue, &mut x);
                for p in paths {
                    let mut b = Board::empty();
                    let mut f = Fumen::default();
                    for m in p {
                        let lc = b.line_clears();
                        b |= m.mask();
                        b = b.clearshift();
                        let page = f.add_page();
                        page.piece = Some(fumen::Piece {
                            kind: match m.piece() {
                                Piece::T => fumen::PieceType::T,
                                Piece::I => fumen::PieceType::I,
                                Piece::J => fumen::PieceType::J,
                                Piece::L => fumen::PieceType::L,
                                Piece::O => fumen::PieceType::O,
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
                    }
                }
            });
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

/// The piece types a `--save` pattern accepts, or `None` when the flag is
/// absent. A goal leaves one piece in hold, so every expansion of the pattern
/// must be a single piece. Exits the process on any other expansion.
fn parse_save(save: Option<&Pattern>) -> Option<Vec<Piece>> {
    let pattern = save?;
    let expansions = pattern.expand();
    if expansions.is_empty() || expansions.iter().any(|e| e.len() != 1) {
        eprintln!(
            "error: --save '{pattern}' must expand to at least one single piece, \
             such as T, [TI] or *"
        );
        std::process::exit(2);
    }
    // The set of expansions holds no duplicate, so the pieces are distinct.
    Some(expansions.into_iter().map(|e| e[0]).collect())
}

/// Returns whether `queue` reaches the goal field from `board`. A `save` set
/// also requires the hold to hold one of its pieces at that moment.
fn has_solve(board: Board, queue: &[Piece], two_l: bool, save: Option<&[Piece]>) -> bool {
    match (save, two_l) {
        (None, false) => jade_solve::reachable(board, queue),
        (None, true) => jade_solve::reachable_2l(board, queue),
        (Some(save), false) => jade_solve::reachable_save(board, queue, save),
        (Some(save), true) => jade_solve::reachable_save_2l(board, queue, save),
    }
}
