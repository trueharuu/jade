pub mod unglue;

use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

use clap::Parser;
use itertools::Itertools;
use jade_core::board::Board;
use jade_pattern::Pattern;
use jade_solve::solve;
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

    /// Decide whether the queue can perform a perfect clear on the given field.
    Solve {
        #[arg(short, long)]
        pattern: Pattern,

        /// Starting field for the solve, as a fumen. Defaults to an empty
        /// field.
        #[arg(short, long)]
        field: Option<String>,

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

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
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
        match self.0 {
            true => write!(f, "use"),
            false => write!(f, "avoid"),
        }
    }
}

impl Deref for Hold {
    type Target = bool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
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

        Command::Solve {
            pattern,
            field,
            hold,
        } => {
            let board = parse_board(field.as_deref());

            let queues = pattern.expand();

            if queues.len() > 1 {
                eprintln!("error: pattern must expand to a single queue");
                std::process::exit(2);
            }

            let queue = &queues.first().unwrap();
            if solve::reachable(board, queue, *hold) {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }

        Command::Percent {
            pattern,
            field,
            hold,
        } => {
            let board = parse_board(field.as_deref());
            let total = pattern.expand().len();

            let queues = pattern.expand();

            let success = queues
                .into_par_iter()
                .map(|queue| solve::reachable(board, &queue, *hold))
                .filter(|x| *x)
                .count();

            println!("{success}/{total}");
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
