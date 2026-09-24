use rayon::prelude::*;

use jade_core::board::Board;
use jade_core::piece::Piece;

use crate::model::Model;
use crate::model::moves;

/// Board reached by locking `m` onto `board`, then clearing completed rows.
#[inline]
#[must_use]
pub fn child(board: Board, m: jade_core::placement::Move) -> Board {
    (board | m.mask()).clearshift()
}

/// Perft node count for `queue`: the number of boards discovered while
/// placing the whole queue from `board`, one count per node in the
/// placement tree. The starting `board` does not count; every board reached
/// by a placement does. With `threads` greater than 1, the walk splits work
/// across the current rayon pool.
#[inline]
#[must_use]
pub fn run(model: Model, board: Board, queue: &[Piece], threads: usize) -> u64 {
    if threads > 1 && queue.len() > 2 {
        run_parallel(model, board, queue)
    } else {
        subtree(model, board, queue)
    }
}

/// Serial perft node count. Each placement from `board` discovers one new
/// board, which adds itself plus its own descendants to the count.
#[inline]
fn subtree(model: Model, board: Board, queue: &[Piece]) -> u64 {
    if board.has_imbalanced_split() || board.has_isolated_cell() {
        return 0;
    }

    let mut total = 0;

    if queue.is_empty() {
        return total;
    }

    for m in &moves(model, queue[0], board) {
        total += 1 + subtree(model, child(board, m), &queue[1..]);
    }
    total
}

/// Parallel two-level split: expand the first two plies into board work
/// units, then walk each unit's subtree on the current rayon pool.
/// This is the mochbot `perft_parallel` shape; memory stays bounded by
/// depth because each subtree walks recursively. First-ply nodes are
/// counted inline; second-ply nodes and their descendants are counted by
/// the parallel walk over the units.
#[inline]
fn run_parallel(model: Model, board: Board, queue: &[Piece]) -> u64 {
    let ml0 = moves(model, queue[0], board);

    let mut units = Vec::new();
    let total = ml0.iter().count() as u64;
    for m0 in &ml0 {
        let b1 = child(board, m0);
        for m1 in &moves(model, queue[1], b1) {
            units.push(child(b1, m1));
        }
    }

    let tail = &queue[2..];
    total + units.len() as u64
        + units
            .par_iter()
            .map(|b| subtree(model, *b, tail))
            .sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_queue;

    fn run_queue(src: &str, threads: usize) -> u64 {
        let queue = parse_queue(src);
        run(Model::Oracle, Board::empty(), &queue, threads)
    }

    // Hand-verified: T has 8+9+8+9 = 34 placements from the empty board,
    // I has 7 horizontal + 10 vertical = 17. Each placement discovers one
    // board; the start board does not count.
    #[test]
    fn single_piece_counts() {
        assert_eq!(run_queue("T", 1), 34);
        assert_eq!(run_queue("I", 1), 17);
    }

    // Frozen oracle node total for the full 7-bag, recorded after the
    // O-spawn height fix. The start board does not count.
    #[test]
    #[ignore = "slow in debug builds"]
    fn full_bag_total() {
        assert_eq!(run_queue("IOLJSZT", 1), 3_390_732);
    }

    #[test]
    fn serial_and_parallel_agree() {
        assert_eq!(run_queue("IOL", 1), run_queue("IOL", 4));
    }
}
