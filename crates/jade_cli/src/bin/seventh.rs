use std::collections::BTreeSet;

use clap::Parser;
use jade_core::{board::Board, queue::Queue};
use jade_pattern::Pattern;
use jade_solve::{Saves, percent::percent};

#[derive(clap::Parser)]
pub struct Program {
    field: Option<String>,
    pattern: Pattern,
    used: Queue,
}

pub fn main() {
    let program = Program::parse();

    let board = parse_board(program.field.as_deref());

    println!("\t'+T\t'+I\t'+J\t'+L\t'+O\t'+S\t'+Z");
    for piece in jade_core::piece::Piece::ALL {
        print!("-{piece}");

        // `pattern`, with `used` removed and `piece` appended to each queue
        let queues = program
            .pattern
            .expand()
            .iter()
            .map(|q| {
                let mut q = q.clone();
                q = q.consume(&program.used.as_slice());
                q.push(piece);
                q
            })
            .collect::<BTreeSet<_>>();

        for other in jade_core::piece::Piece::ALL {
            // get chance that field solves pattern with `other` saved
            let mut save = Saves::empty();
            save.add(other);
            let (n, d) = percent(board, &queues, false, save, true);
                let p = n as f64 / d as f64;
                let mp = minimum_precision(d) - 2;
                print!("\t{:.mp$}%", p * 100.0);
        }
        println!();
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
