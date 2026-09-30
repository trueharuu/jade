use clap::Parser;
use itertools::Itertools;
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::render;
use jade_nav::oracle;
use jade_pattern::Pattern;

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
        queue: Pattern,
        /// Starting field for the solve, as a fumen. Defaults to an empty
        /// field.
        #[arg(short, long)]
        field: Option<String>,
        /// Count a two-line (`PC_2`) field as success instead of the full
        /// field.
        #[arg(long = "2l")]
        two_l: bool,
        /// The required saves for a queue to be a matched solve.
        #[arg(short, long)]
        save: Option<Pattern>,
    },

    Move {
        #[arg(short, long, default_value_t = String::from("v115@vhAAgh"))]
        field: String,
        piece: Piece,
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
        /// The required saves for a queue to be a matched solve.
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
            queue,
            field,
            two_l,
            save,
        } => {
            let board = parse_board(field.as_deref());

            // A field that can never be filled has no solution. Reject it
            // before the search, which would otherwise prune the root anyway.
            if jade_solve::is_unfillable(board) {
                // return failure exit code
                std::process::exit(1);
            }

            let yes = queue.expand().iter().any(|q| has_solve(board, q, two_l));

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
            let queues = pattern.expand();
            let total = queues.len();

            // A field that can never be filled has no solution for any queue,
            // so every queue fails. Report that without running the search.
            if jade_solve::is_unfillable(board) {
                println!("0/{total}");
                std::process::exit(0);
            }

            let solved = queues.iter().filter(|q| has_solve(board, q, two_l)).count();

            println!("{solved}/{total}");
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

/// Returns whether `queue` reaches the goal field from `board`.
fn has_solve(board: Board, queue: &[Piece], two_l: bool) -> bool {
    if two_l {
        jade_solve::reachable_2l(board, queue)
    } else {
        jade_solve::reachable(board, queue)
    }
}
