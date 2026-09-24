use std::time::Instant;

use clap::Parser;
use itertools::Itertools;
use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_pattern::Pattern;
use rayon::ThreadPoolBuilder;

use jade_perft::human;
use jade_perft::model::Model;
use jade_perft::perft;

#[derive(clap::Parser)]
#[command(name = "jade_perft", about = "move-generation perft and timing")]
struct Program {
    /// Queue pattern in the jade pattern grammar. Every expansion is one
    /// queue, run serially in its own section.
    queue: Pattern,

    /// Move generator to time.
    #[arg(short, long, value_enum, default_value_t)]
    model: Model,

    /// Compare oracle vs fast move counts for each queue.
    #[arg(short, long)]
    compare: bool,

    /// Threads for the parallel walk; 1 runs serially.
    #[arg(short, long, default_value_t = std::thread::available_parallelism().map_or(1, |n| n.get()))]
    threads: usize,

    /// Place only the first N pieces of each queue.
    #[arg(short, long)]
    depth: Option<usize>,

    /// Starting field for the perft run. Defaults to an empty field.
    #[arg(short, long, default_value_t = String::from("v115@vhAAgh"))]
    field: String,
}

fn main() {
    let program = Program::parse();

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

    let board = match jade_perft::parse_fumen(&program.field) {
        Ok(board) => board,
        Err(msg) => {
            eprintln!("error: failed to parse fumen: {msg}");
            std::process::exit(1);
        }
    };

    // println!("{}", render::board(&board));

    pool.install(|| {
        for queue in &queues {
            let depth = match depth_for(queue, program.depth) {
                Ok(depth) => depth,
                Err(msg) => {
                    eprintln!("{msg}");
                    std::process::exit(1);
                }
            };
            if program.compare {
                report_compare(board, &queue[..depth], program.threads);
            } else {
                report(program.model, board, &queue[..depth], program.threads);
            }
        }
    });
}

fn report_compare(board: Board, queue: &[Piece], threads: usize) {
    let name = queue.iter().map(ToString::to_string).join("");

    let oracle_n = perft::run(Model::Oracle, board, queue, threads);
    let fast_n = perft::run(Model::Fast, board, queue, threads);

    let diff = fast_n as i64 - oracle_n as i64;
    let diff_str = if diff >= 0 {
        format!("+{diff}")
    } else {
        format!("-{}", diff.abs())
    };

    println!("perft({name}) = {diff_str} ({oracle_n} -> {fast_n})");
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
