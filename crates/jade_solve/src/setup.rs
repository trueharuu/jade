use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::queue::Queue;
use jade_pattern::disjoint::DisjointPattern;

use crate::percent::children;
use crate::rules::piece_from_index;

/// The board that setups are built on.
const START: Board = Board(0);

/// Generates all PC-feasible `n`-piece setups for a given pattern, along with
/// the set of pieces that each one used.
///
/// A setup is a board that `n` pieces can build from the start board and that
/// pruning does not reject. A board that several piece sets can build appears
/// once for each set. The pieces used are in order of piece index.
///
/// For `DisjointPattern::Mixed`, no rules apply. For
/// `DisjointPattern::Disjoint`, the left-hand pattern must use all of its
/// pieces within the setup.
///
/// Returns an empty list if `n` is 0 or more than 10, or if no queue can build
/// an `n`-piece setup (for example, `n` is below the length of the left
/// pattern).
pub fn setups(pattern: DisjointPattern, n: usize, hold: bool) -> Vec<(Board, Queue)> {
    if n == 0 || n > 10 {
        return Vec::new();
    }

    // Group the placement orders by the set of pieces placed.
    let mut groups: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for (prefix, split) in prefixes(&pattern, n) {
        schedules(&prefix, n, hold, 0, None, 0, 0, &mut |code, hold_idx, d| {
            // All pieces of the left part must be placed. Queue indices below
            // `d` are consumed. Only the hold piece among them is not placed.
            if split != 0 && !(d >= split && hold_idx.map_or(true, |h| h >= split)) {
                return;
            }
            groups.entry(multiset_key(code, n)).or_default().push(code);
        });
    }

    // Board sets of placement prefixes, shared by all piece sets.
    let mut memo = Memo::new();
    memo.insert((0, 0), vec![START.0]);

    let mut out = Vec::new();
    for (key, mut orders) in groups {
        orders.sort_unstable();
        orders.dedup();
        let used = decode_multiset(key);
        for b in setup_boards(n, &orders, &mut memo) {
            out.push((Board(b), queue_from(&used)));
        }
    }
    out
}

/// Builds a `Queue` from pieces.
///
/// This is the only place that builds a `Queue`. It assumes a `push` method.
/// Change it if `Queue` is built another way.
fn queue_from(pieces: &[Piece]) -> Queue {
    let mut q = Queue::new();
    for &p in pieces {
        q.push(p);
    }
    q
}

/// Returns the distinct first `n + 1` pieces of the queues of `pattern`, each
/// with the length of its left part (0 for a mixed pattern). Only these pieces
/// can take part in a setup.
fn prefixes(pattern: &DisjointPattern, n: usize) -> BTreeSet<(Vec<Piece>, usize)> {
    let cut = |mut q: Vec<Piece>| {
        q.truncate(n + 1);
        q
    };
    match pattern {
        DisjointPattern::Mixed(p) => p.expand().into_iter().map(|q| (cut(q.to_vec()), 0)).collect(),
        DisjointPattern::Disjoint(l, r) => {
            let (l, r) = (l.expand(), r.expand());
            let mut out = BTreeSet::new();
            for a in &l {
                for b in &r {
                    let mut q = a.clone();
                    q.extend(b);
                    out.insert((cut(q.to_vec()), a.len()));
                }
            }
            out
        }
    }
}

/// Enumerates the ways to place `n` pieces from `queue`. It calls `f` with the
/// order code (3 bits per piece, first placed piece in the highest digit), the
/// queue index of the hold piece, and the number of consumed queue pieces.
///
/// The options are the same as in `Solver::search`, except that "place the hold
/// piece" is never skipped. It differs from "place the active piece" in which
/// queue index stays unplaced, and the left part of a disjoint pattern needs
/// that index.
#[allow(clippy::too_many_arguments)]
fn schedules<F: FnMut(u64, Option<usize>, usize)>(
    queue: &[Piece],
    n: usize,
    hold_on: bool,
    d: usize,
    hold: Option<usize>,
    code: u64,
    placed: usize,
    f: &mut F,
) {
    if placed == n {
        f(code, hold, d);
        return;
    }
    let Some(&active) = queue.get(d) else {
        return;
    };
    schedules(
        queue,
        n,
        hold_on,
        d + 1,
        hold,
        (code << 3) | active as u64,
        placed + 1,
        f,
    );
    if !hold_on {
        return;
    }
    match hold {
        Some(h) => {
            schedules(
                queue,
                n,
                hold_on,
                d + 1,
                Some(d),
                (code << 3) | queue[h] as u64,
                placed + 1,
                f,
            );
        }
        None => {
            if d + 1 < queue.len() {
                schedules(
                    queue,
                    n,
                    hold_on,
                    d + 2,
                    Some(d),
                    (code << 3) | queue[d + 1] as u64,
                    placed + 1,
                    f,
                );
            }
        }
    }
}

/// Counts of each piece in an order code, 4 bits per piece.
fn multiset_key(code: u64, n: usize) -> u64 {
    (0..n).fold(0, |key, i| key + (1u64 << (4 * ((code >> (3 * i)) & 7))))
}

/// Returns the pieces of a multiset key, in order of piece index.
fn decode_multiset(key: u64) -> Vec<Piece> {
    let mut out = Vec::new();
    for p in 0..7u8 {
        for _ in 0..((key >> (4 * p)) & 0xf) {
            out.push(piece_from_index(p));
        }
    }
    out
}

/// Board sets of placement prefixes, keyed by `(length, prefix value)`. The
/// prefix value is the first `length` digits of an order code.
type Memo = HashMap<(usize, u64), Vec<u64>>;

/// Makes sure that `memo` has the board sets of all prefixes of `code`, up to
/// the length `len`. `code` has `n` digits, and the first piece is the highest.
fn ensure_prefix(memo: &mut Memo, n: usize, code: u64, len: usize) {
    for l in 1..=len {
        let key = (l, code >> (3 * (n - l)));
        if memo.contains_key(&key) {
            continue;
        }
        let parent = (l - 1, code >> (3 * (n - l + 1)));
        let piece = piece_from_index(((code >> (3 * (n - l))) & 7) as u8);
        let boards = children(&memo[&parent], piece);
        memo.insert(key, boards);
    }
}

/// Returns the sorted, unique boards after placing any of `orders` (unique
/// orders of `n` pieces) from the start board. Pruned boards are dropped at
/// every depth.
///
/// Orders that end in the same piece share one last step. The board sets of
/// their first `n - 1` placements are joined first, then the last piece is
/// placed once on each distinct board. Placing a piece on a set of boards is
/// the same as placing it on each board, so the result does not change. The
/// prefix sets are also shared between piece sets through `memo`. The memo
/// holds one board set for each prefix up to the length `n - 1`, so it grows
/// quickly with `n`.
fn setup_boards(n: usize, orders: &[u64], memo: &mut Memo) -> Vec<u64> {
    let mut by_last: [Vec<u64>; 7] = Default::default();
    for &code in orders {
        by_last[(code & 7) as usize].push(code >> 3);
    }

    let mut out = Vec::new();
    for (last, prefixes) in by_last.iter().enumerate() {
        if prefixes.is_empty() {
            continue;
        }
        let mut parents: Vec<u64> = Vec::new();
        for &prefix in prefixes {
            ensure_prefix(memo, n, prefix << 3, n - 1);
            parents.extend_from_slice(&memo[&(n - 1, prefix)]);
        }
        parents.sort_unstable();
        parents.dedup();
        out.extend(children(&parents, piece_from_index(last as u8)));
    }
    out.sort_unstable();
    out.dedup();
    out
}
