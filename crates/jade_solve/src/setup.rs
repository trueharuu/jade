use std::collections::{BTreeMap, BTreeSet};

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::queue::Queue;
use jade_pattern::disjoint::DisjointPattern;
use rustc_hash::FxHashMap;

use crate::percent::children;
use crate::rules::piece_from_index;

/// The board that setups are built on.
const START: Board = Board(0);

/// The bit that holds the queue length in an encoded queue. A queue holds at
/// most `n + 1` pieces and `n` is at most 10, so the pieces need 33 bits and
/// the length needs 4.
const LEN_SHIFT: u32 = 33;

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
pub fn setups(pattern: DisjointPattern, n: usize, hold: bool) -> BTreeMap<Queue, Vec<Board>> {
    if n == 0 || n > 10 {
        return BTreeMap::new();
    }

    // Every placement order, with the multiset key of its pieces. One sort
    // gives both the groups and the sorted codes inside each group.
    let mut pairs: Vec<(u64, u64)> = Vec::new();
    for (queue, split) in prefixes(&pattern, n) {
        schedules(
            queue,
            hold,
            0,
            None,
            0,
            0,
            n,
            &mut |key, code, hold_idx, d| {
                // All pieces of the left part must be placed. Queue indices
                // below `d` are consumed. Only the hold piece among them is not
                // placed.
                if split != 0 && !(d >= split && hold_idx.map_or(true, |h| h >= split)) {
                    return;
                }
                pairs.push((key, code));
            },
        );
    }
    pairs.sort_unstable();

    // Board sets of placement prefixes, shared by all piece sets.
    let mut memo = Memo::new();
    let mut scratch = Scratch::default();
    let mut orders: Vec<u64> = Vec::new();

    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < pairs.len() {
        // The codes of one piece set are a run of equal keys. The sort made
        // the run sorted by code, so a dedup is all that the run needs.
        let key = pairs[i].0;
        let mut j = i + 1;
        while j < pairs.len() && pairs[j].0 == key {
            j += 1;
        }
        orders.clear();
        orders.extend(pairs[i..j].iter().map(|&(_, code)| code));
        orders.dedup();

        let mut buf = [Piece::T; 10];
        let len = decode_multiset(key, &mut buf);
        let queue = queue_from(&buf[..len]);
        for &b in setup_boards(n, &orders, &mut memo, &mut scratch) {
            out.entry(queue).or_insert_with(Vec::new).push(Board(b));
        }
        i = j;
    }
    out
}

/// Builds a `Queue` from pieces.
///
/// This is the only place that builds a `Queue`. It assumes a `from_slice`
/// method. Change it if `Queue` is built another way.
const fn queue_from(pieces: &[Piece]) -> Queue {
    Queue::from_slice(pieces)
}

/// Returns the distinct first `n + 1` pieces of the queues of `pattern`, each
/// with the length of its left part (0 for a mixed pattern). Only these pieces
/// can take part in a setup.
///
/// Each queue is one `u64`: the first `n + 1` pieces, 3 bits per piece with
/// the first piece in the highest digit, then the queue length above bit
/// [`LEN_SHIFT`].
fn prefixes(pattern: &DisjointPattern, n: usize) -> BTreeSet<(u64, usize)> {
    let cut = |q: &Queue| encode(q.prefix(n + 1));
    match pattern {
        DisjointPattern::Mixed(p) => p.expand().into_iter().map(|q| (cut(&q), 0)).collect(),
        DisjointPattern::Disjoint(l, r) => {
            let (l, r) = (l.expand(), r.expand());
            let mut out = BTreeSet::new();
            for a in &l {
                for b in &r {
                    let mut q = a.clone();
                    q.extend(b);
                    out.insert((cut(&q), a.len()));
                }
            }
            out
        }
    }
}

/// Packs a queue into a `u64`. See [`prefixes`] for the layout.
fn encode(pieces: &[Piece]) -> u64 {
    let mut code = 0;
    for &p in pieces {
        code = (code << 3) | p as u64;
    }
    code | ((pieces.len() as u64) << LEN_SHIFT)
}

/// Returns the number of pieces in an encoded queue.
#[inline(always)]
const fn q_len(queue: u64) -> usize {
    (queue >> LEN_SHIFT) as usize
}

/// Returns the piece at index `i` of an encoded queue.
#[inline(always)]
const fn q_get(queue: u64, i: usize) -> Piece {
    piece_from_index(((queue >> (3 * (q_len(queue) - 1 - i))) & 7) as u8)
}

/// Enumerates the ways to place `left` pieces from `queue`. It calls `f` with
/// the multiset key of the placed pieces, the order code (3 bits per piece,
/// first placed piece in the highest digit), the queue index of the hold
/// piece, and the number of consumed queue pieces.
///
/// The options are the same as in `Solver::search`, except that "place the hold
/// piece" is never skipped. It differs from "place the active piece" in which
/// queue index stays unplaced, and the left part of a disjoint pattern needs
/// that index.
#[allow(clippy::too_many_arguments)]
fn schedules<F: FnMut(u64, u64, Option<usize>, usize)>(
    queue: u64,
    hold_on: bool,
    d: usize,
    hold: Option<usize>,
    code: u64,
    key: u64,
    left: usize,
    f: &mut F,
) {
    if left == 0 {
        f(key, code, hold, d);
        return;
    }
    let len = q_len(queue);
    if d >= len {
        return;
    }
    let active = q_get(queue, d) as u64;
    schedules(
        queue,
        hold_on,
        d + 1,
        hold,
        (code << 3) | active,
        key + (1 << (4 * active)),
        left - 1,
        f,
    );
    if !hold_on {
        return;
    }
    match hold {
        Some(h) => {
            let piece = q_get(queue, h) as u64;
            schedules(
                queue,
                hold_on,
                d + 1,
                Some(d),
                (code << 3) | piece,
                key + (1 << (4 * piece)),
                left - 1,
                f,
            );
        }
        None => {
            if d + 1 < len {
                let piece = q_get(queue, d + 1) as u64;
                schedules(
                    queue,
                    hold_on,
                    d + 2,
                    Some(d),
                    (code << 3) | piece,
                    key + (1 << (4 * piece)),
                    left - 1,
                    f,
                );
            }
        }
    }
}

/// Writes the pieces of a multiset key into `buf`, in order of piece index,
/// and returns their count. `buf` must hold 10 pieces, the most that a setup
/// can place.
fn decode_multiset(key: u64, buf: &mut [Piece; 10]) -> usize {
    let mut len = 0;
    for p in 0..7u8 {
        for _ in 0..((key >> (4 * p)) & 0xf) {
            buf[len] = piece_from_index(p);
            len += 1;
        }
    }
    len
}

/// Board sets of placement prefixes. A key holds the prefix value of the first
/// `len` digits of an order code, shifted up by 4, and `len` in the low 4
/// bits. The prefix has at most 9 digits, so the key stays below 31 bits. The
/// sets are held in one arena, so a new prefix costs one board set, not a new
/// map entry and a new box.
struct Memo {
    index: FxHashMap<u64, u32>,
    sets: Vec<Vec<u64>>,
}

impl Memo {
    /// Creates a memo that holds only the start board, at length 0.
    fn new() -> Self {
        Self {
            index: FxHashMap::from_iter([(Self::key(0, 0), 0)]),
            sets: vec![vec![START.0]],
        }
    }

    /// Returns the key of the prefix value `prefix` at length `len`.
    #[inline(always)]
    const fn key(len: usize, prefix: u64) -> u64 {
        (prefix << 4) | len as u64
    }

    /// Makes sure that the memo has the board set of every prefix of `code`, up
    /// to the length `len`, and returns the arena index of the length-`len`
    /// set. `code` has `n` digits and its first piece is the highest.
    fn ensure(&mut self, n: usize, code: u64, len: usize) -> u32 {
        let top = Self::key(len, code >> (3 * (n - len)));
        if let Some(&i) = self.index.get(&top) {
            return i;
        }
        // The set of length 0 is always present, so the first lookup cannot
        // fail.
        let mut parent = 0;
        for l in 1..=len {
            let digit = code >> (3 * (n - l));
            let key = Self::key(l, digit);
            if let Some(&i) = self.index.get(&key) {
                parent = i;
                continue;
            }
            let piece = piece_from_index((digit & 7) as u8);
            let boards = children(&self.sets[parent as usize], piece);
            self.sets.push(boards);
            let i = (self.sets.len() - 1) as u32;
            self.index.insert(key, i);
            parent = i;
        }
        parent
    }
}

/// Buffers that `setup_boards` reuses across piece sets.
#[derive(Default)]
struct Scratch {
    /// The `n - 1` piece prefixes of one group, split by the last piece.
    by_last: [Vec<u64>; 7],
    /// The arena indices of the parent sets, one list per last piece.
    parents: [Vec<u32>; 7],
    /// The parent sets of the current group, sorted and unique.
    merged: Vec<u64>,
    /// The boards after the last piece, one set per last piece.
    step: [Vec<u64>; 7],
    /// The result for the current group.
    out: Vec<u64>,
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
fn setup_boards<'a>(n: usize, orders: &[u64], memo: &'a mut Memo, s: &'a mut Scratch) -> &'a [u64] {
    for prefixes in &mut s.by_last {
        prefixes.clear();
    }
    for &code in orders {
        s.by_last[(code & 7) as usize].push(code >> 3);
    }

    // `step` must lose the sets of the last group. A piece set with no order
    // ending in a piece skips the loop below, so a stale set would stay in the
    // result.
    for last in 0..7 {
        s.parents[last].clear();
        s.step[last].clear();
    }

    // The memo must grow before anything reads from it. A borrow of the memo
    // lasts until the end of the phase that made it, so the fill is its own
    // loop.
    for (last, prefixes) in s.by_last.iter().enumerate() {
        s.parents[last].extend(prefixes.iter().map(|&p| memo.ensure(n, p << 3, n - 1)));
    }

    s.out.clear();
    for last in 0..7 {
        if s.parents[last].is_empty() {
            continue;
        }
        // The union of the parent sets. A heap merge of the sorted sets costs
        // more than one sort of the joined values, because the join has many
        // lists and the sort is a fast pass over the values.
        s.merged.clear();
        // `merged` keeps the capacity of the largest group, so this reserve
        // is free after the first group.
        s.merged
            .reserve(s.parents[last].iter().map(|&i| memo.sets[i as usize].len()).sum());
        for &i in &s.parents[last] {
            s.merged.extend_from_slice(&memo.sets[i as usize]);
        }
        s.merged.sort_unstable();
        s.merged.dedup();
        s.step[last] = children(&s.merged, piece_from_index(last as u8));
    }

    s.out.extend(s.step.iter().flatten().copied());
    s.out.sort_unstable();
    s.out.dedup();
    &s.out
}
