use jade_core::board::Board;
use jade_core::piece::Piece;
use rayon::prelude::*;

use crate::moves::moves;
use crate::rules::PC_2;
use crate::rules::PC_4;
use crate::rules::completes_goal;
use crate::rules::outside_playfield;
use crate::rules::piece_from_index;
use crate::rules::pruned;
use crate::saves::Saves;
use crate::saves::save_ok;

/// One goal board and the data needed to solve it.
struct Target {
    goal: Board,
    /// Number of pieces to place from the start board.
    k: usize,
    /// Sorted codes of the orders that reach `goal` with no hold.
    winners: Vec<u64>,
}

/// Counts the queues for which [`crate::solve::reachable`] returns `true`.
///
/// Returns `(successes, total)`. The arguments have the meaning they have in
/// `reachable`. `board` must be unchanged by `clearshift`.
///
/// Hold only changes the order of the placed pieces and the leftover piece. So
/// the search runs with no hold, over the placement orders that the queues can
/// produce. Orders share prefixes, and a prefix is solved once. Queues are cut
/// to the first `k + 1` pieces, because later pieces cannot matter.
#[must_use]
pub fn percent<I, Q>(board: Board, queues: I, two_l: bool, saves: Saves, hold: bool) -> (u64, u64)
where
    I: IntoIterator<Item = Q>,
    Q: AsRef<[Piece]>,
{
    debug_assert!(board.clearshift() == board, "board is not clearshifted");

    let cells = board.0.count_ones();
    let mut targets: Vec<Target> = Vec::with_capacity(2);
    if cells % 4 == 0 {
        if cells <= 40 {
            targets.push(Target {
                goal: PC_4,
                k: ((40 - cells) / 4) as usize,
                winners: Vec::new(),
            });
        }
        if two_l && cells <= 20 {
            targets.push(Target {
                goal: PC_2,
                k: ((20 - cells) / 4) as usize,
                winners: Vec::new(),
            });
        }
    }

    // Only the first `k + 1` pieces matter. `PC_4` has the largest `k`.
    let keep = targets.first().map_or(0, |t| t.k + 1);

    // Group equal truncated queues. A key is (3 bits per piece, length).
    let mut total = 0u64;
    let mut keys: Vec<(u64, u8)> = Vec::new();
    for q in queues {
        total += 1;
        if targets.is_empty() {
            continue;
        }
        let q = q.as_ref();
        let n = q.len().min(keep);
        let code = q[..n].iter().fold(0u64, |c, &p| (c << 3) | p as u64);
        keys.push((code, n as u8));
    }
    if targets.is_empty() {
        return (0, total);
    }
    keys.par_sort_unstable();
    let mut unique: Vec<((u64, u8), u64)> = Vec::new();
    for key in keys {
        match unique.last_mut() {
            Some((last, count)) if *last == key => *count += 1,
            _ => unique.push((key, 1)),
        }
    }

    // Collect the orders that some schedule can produce, then solve them.
    for t in &mut targets {
        let (k, goal) = (t.k, t.goal);
        let mut needed: Vec<u64> = unique
            .par_iter()
            .flat_map_iter(|&((code, n), _)| {
                let mut buf = [Piece::T; 12];
                let q = decode(code, n as usize, &mut buf);
                let mut out = Vec::new();
                walk(q, k, hold, 0, None, 0, 0, &mut |order, left| {
                    if save_ok(saves, left, None) {
                        out.push(order);
                    }
                    false
                });
                out
            })
            .collect();
        needed.par_sort_unstable();
        needed.dedup();
        t.winners = solve_orders(board, goal, k, &needed);
    }

    // A queue succeeds if some schedule gives a winning order and a good leftover.
    let successes: u64 = unique
        .par_iter()
        .map(|&((code, n), count)| {
            let mut buf = [Piece::T; 12];
            let q = decode(code, n as usize, &mut buf);
            let ok = targets.iter().any(|t| {
                walk(q, t.k, hold, 0, None, 0, 0, &mut |order, left| {
                    save_ok(saves, left, None) && t.winners.binary_search(&order).is_ok()
                })
            });
            if ok {
                count
            } else {
                0
            }
        })
        .sum();
    (successes, total)
}

/// Decodes a queue key into pieces.
fn decode(code: u64, n: usize, buf: &mut [Piece; 12]) -> &[Piece] {
    for i in 0..n {
        buf[i] = piece_from_index(((code >> (3 * (n - 1 - i))) & 7) as u8);
    }
    &buf[..n]
}

/// Enumerates the hold schedules of `queue` that place `k` pieces. It calls `f`
/// with the order code and the leftover piece. If `f` returns `true`, the walk
/// stops and returns `true`.
///
/// The three options are the ones in `Solver::search`:
/// 1. Place the active piece.
/// 2. Place the hold piece (skipped if it equals the active piece).
/// 3. If hold is empty, hold the active piece and place the next piece.
///
/// The two must stay in step with each other. They are not merged because
/// `Solver::search` carries a board and a failure cache and this carries an
/// order code.
#[allow(clippy::too_many_arguments)]
fn walk<F: FnMut(u64, Option<Piece>) -> bool>(
    queue: &[Piece],
    k: usize,
    hold_on: bool,
    d: usize,
    hold: Option<Piece>,
    code: u64,
    placed: usize,
    f: &mut F,
) -> bool {
    if placed == k {
        return f(code, hold.or(queue.get(d).copied()));
    }
    let Some(&active) = queue.get(d) else {
        return false;
    };
    if walk(queue, k, hold_on, d + 1, hold, (code << 3) | active as u64, placed + 1, f) {
        return true;
    }
    if !hold_on {
        return false;
    }
    match hold {
        Some(h) => {
            h != active
                && walk(queue, k, hold_on, d + 1, Some(active), (code << 3) | h as u64, placed + 1, f)
        }
        None => {
            d + 1 < queue.len()
                && walk(
                    queue,
                    k,
                    hold_on,
                    d + 2,
                    Some(active),
                    (code << 3) | queue[d + 1] as u64,
                    placed + 1,
                    f,
                )
        }
    }
}

/// Returns the sorted subset of `codes` (orders of `k` pieces, sorted, unique)
/// that reach `goal` from `board` with no hold.
fn solve_orders(board: Board, goal: Board, k: usize, codes: &[u64]) -> Vec<u64> {
    if codes.is_empty() {
        return Vec::new();
    }
    if k == 0 {
        return if board == goal { codes.to_vec() } else { Vec::new() };
    }
    if pruned(board) {
        return Vec::new();
    }
    expand(&[board.0], codes, 0, k, goal)
}

/// Groups with at least this many codes may run in parallel.
const PAR_MIN_CODES: usize = 32;
/// Child groups run in parallel only at depths below this value.
const PAR_DEPTH: usize = 3;

/// Expands one trie node and returns the sorted winning codes below it. `boards`
/// is the sorted, unique set of boards after the prefix of length `depth`. All
/// `codes` share that prefix.
///
/// The child groups are independent. At shallow depths they run in parallel.
/// Each group returns its own vector. The vectors join in group order, so the
/// result stays sorted.
fn expand(boards: &[u64], codes: &[u64], depth: usize, k: usize, goal: Board) -> Vec<u64> {
    let shift = 3 * (k - 1 - depth);
    let digit = |c: u64| ((c >> shift) & 7) as u8;

    // Contiguous ranges of codes that have the same next piece.
    let mut groups: Vec<(usize, usize)> = Vec::with_capacity(7);
    let mut lo = 0;
    while lo < codes.len() {
        let d = digit(codes[lo]);
        let mut hi = lo + 1;
        while hi < codes.len() && digit(codes[hi]) == d {
            hi += 1;
        }
        groups.push((lo, hi));
        lo = hi;
    }

    let run = |&(a, b): &(usize, usize)| -> Vec<u64> {
        let piece = piece_from_index(digit(codes[a]));

        if depth + 1 == k {
            // Last piece. The codes are unique, so this group has one code.
            return if boards
                .iter()
                .any(|&bd| completes_goal(Board(bd), piece, goal))
            {
                vec![codes[a]]
            } else {
                Vec::new()
            };
        }

        let mut next: Vec<u64> = Vec::new();
        for &bd in boards {
            let bd = Board(bd);
            for m in &moves(bd, piece) {
                let mut n = bd;
                n |= m.mask();
                n = n.clearshift();
                // Bits at or above bit 40 are outside the playfield.
                if outside_playfield(n) || pruned(n) {
                    continue;
                }
                next.push(n.0);
            }
        }
        next.sort_unstable();
        next.dedup();
        if next.is_empty() {
            Vec::new()
        } else {
            expand(&next, &codes[a..b], depth + 1, k, goal)
        }
    };

    if depth < PAR_DEPTH && groups.len() > 1 && codes.len() >= PAR_MIN_CODES {
        groups
            .par_iter()
            .map(run)
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect()
    } else {
        groups.iter().flat_map(run).collect()
    }
}
