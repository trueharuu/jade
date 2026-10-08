use std::collections::BTreeSet;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

use clap::Parser;

use itertools::Itertools;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_pattern::Pattern;
use jade_pattern::disjoint::DisjointPattern;
use jade_solve::evaluate::Evaluator;
use jade_solve::fumen::encode_fumen;
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
        #[arg(short, long, default_value_t = 1.0)]
        cutoff: f64,

        /// Require the hold to hold a piece of this pattern when the field
        /// fills. Every expansion must be a single piece.
        #[arg(short, long, default_value_t = Saves::empty())]
        save: Saves,

        /// Whether to support hold.
        #[arg(long, default_value_t = Hold(true))]
        hold: Hold,
    },

    Saves {
        #[arg(short, long)]
        pattern: Pattern,

        /// Starting field for the solve, as a fumen. Defaults to an empty
        /// field.
        #[arg(short, long)]
        field: Option<String>,

        /// The save criteria to consider. Defaults to all pieces.
        #[arg(short, long, default_value_t = Saves::all())]
        save: Saves,
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
            } else {
                std::process::exit(1);
            }
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
            hold,
            save,
        } => {
            let ss = setups(pattern.clone(), n, *hold);

            eprintln!(
                "{} ({})",
                ss.values().map(|x| x.len()).sum::<usize>(),
                ss.keys().join(" ")
            );

            let pat = pattern.join().expand();

            // A cutoff of 1.0 means "only setups that always clear", which is
            // the threshold `percent` applied per board. Below 1.0 the setup
            // just has to reach that rate.
            let evaluator = Evaluator::new(Some(cutoff));

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
                    let p = n as f64 / d as f64;
                    let mp = minimum_precision(d);
                    println!("{}\t{n}\t{d}\t{p:.mp$}\t{queue}", encode_fumen(&board));
                }
            }
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
                println!("{piece}: {n}/{d}");
            }
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

/// Returns the minimum precision such that no two values with denominator
/// `denom` map to the same value.
fn minimum_precision(denom: u64) -> usize {
    denom.ilog10() as usize + 1
}
