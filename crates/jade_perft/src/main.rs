use std::time::Instant;

use clap::Parser;
use itertools::Itertools;
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_pattern::Pattern;
use rayon::ThreadPoolBuilder;

use jade_perft::model::Model;
use jade_perft::{human, perft};

#[derive(clap::Parser)]
#[command(name = "jade_perft", about = "move-generation perft and timing")]
struct Program {
    /// Queue pattern in the jade pattern grammar. Every expansion is one
    /// queue, run serially in its own section.
    queue: Pattern,

    /// Move generator to time.
    #[arg(long, value_enum, default_value_t)]
    model: Model,

    /// Threads for the parallel walk; 1 runs serially.
    #[arg(long, default_value_t = 1)]
    threads: usize,

    /// Place only the first N pieces of each queue.
    #[arg(long)]
    depth: Option<usize>,
}

fn main() {
    let program = Program::parse();

    if program.model == Model::Fast {
        eprintln!("error: model `fast` is not implemented yet; use `--model oracle`");
        std::process::exit(1);
    }

    let queues = program.queue.expand();
    let pool = match ThreadPoolBuilder::new()
        .num_threads(program.threads)
        .build()
    {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("error: failed to build thread pool: {err}");
            std::process::exit(1);
        }
    };

    pool.install(|| {
        let board = Board::empty();
        for queue in &queues {
            let depth = match depth_for(queue, program.depth) {
                Ok(depth) => depth,
                Err(msg) => {
                    eprintln!("{msg}");
                    std::process::exit(1);
                }
            };
            report(program.model, board, &queue[..depth], program.threads);
        }
    });
}

/// The perft depth for one queue expansion: the queue length, or the
/// `--depth` prefix when given.
fn depth_for(queue: &[Piece], depth: Option<usize>) -> Result<usize, String> {
    match depth {
        Some(d) if d > queue.len() => Err(format!(
            "error: depth {d} exceeds queue length {}",
            queue.len()
        )),
        Some(d) => Ok(d),
        None => Ok(queue.len()),
    }
}

fn report(model: Model, board: Board, queue: &[Piece], threads: usize) {
    let name = queue.iter().map(ToString::to_string).join("");

    let t0 = Instant::now();
    let n = perft::run(model, board, queue, threads);
    let elapsed = t0.elapsed();

    let secs = elapsed.as_secs_f64();
    let rate = if secs > 0.0 {
        human(n as f64 / secs)
    } else {
        "0".to_string()
    };
    println!("perft({name}) = {n} in {elapsed:?} ({rate} nodes/s)");
}
