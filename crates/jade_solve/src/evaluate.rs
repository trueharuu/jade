use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

use jade_core::board::Board;
use jade_core::piece::Piece;
use rayon::prelude::*;

use crate::Saves;
use crate::percent::children;
use crate::rules::piece_from_index;
use crate::saves::save_ok;
use crate::solve::PC_4;
use crate::solve::moves;
use crate::solve::reachable;

/// The most queues that a board tests one by one before the trie decides.
const PROBE_LIMIT: usize = 64;
/// The number of queue slots that the threads of one group share.
const HOT: usize = 16;
/// The most failed queues that one trie run adds to the hot slots.
const LEARN: usize = 2;
const EMPTY: u32 = u32::MAX;

/// Evaluates setups against a cutoff, exactly.
///
/// A setup is reported if its success count is at least the cutoff. A board is
/// rejected as soon as the queues that failed on it outweigh what the cutoff
/// allows. Only queues that really failed are counted, so no setup that reaches
/// the cutoff is lost.
///
/// With no cutoff, the threshold is the rate of the `limit`th best setup seen
/// so far. Use [`keep`] on the results at the end, because that rate can grow
/// during the run.
///
/// [`keep`]: Evaluator::keep
pub struct Evaluator {
    cutoff: Option<f64>,
    /// The best rates seen so far, sorted from best to worst. The vector holds
    /// at most `limit` entries. With no cutoff it is the only shared state, so
    /// it needs a lock.
    top: Mutex<Vec<f64>>,
    /// How many rates `top` keeps. A limit of 1 means "the best rate only".
    limit: usize,
}

impl Evaluator {
    pub fn new(cutoff: Option<f64>) -> Self {
        Self::with_limit(cutoff, 1)
    }

    /// Makes an evaluator that keeps the `limit` best rates when there is no
    /// cutoff. The limit has no effect on a cutoff run.
    pub fn with_limit(cutoff: Option<f64>, limit: usize) -> Self {
        Self {
            cutoff,
            top: Mutex::new(Vec::new()),
            limit: limit.max(1),
        }
    }

    /// The rate that a setup must reach to be reported, or `None` for a cutoff
    /// run. A run with fewer than `limit` results has no threshold yet.
    fn threshold(&self) -> Option<f64> {
        if self.cutoff.is_some() {
            return None;
        }
        let top = self.top.lock().unwrap();
        (top.len() == self.limit).then(|| top[top.len() - 1])
    }

    /// Records a rate, keeping the best `limit` of them.
    fn record(&self, rate: f64) {
        let mut top = self.top.lock().unwrap();
        top.push(rate);
        // Most insertions are worse than the last entry and go straight back
        // out, so only the tail of the vector moves.
        top.sort_unstable_by(|a, b| b.total_cmp(a));
        top.truncate(self.limit);
    }

    /// The fewest successes out of `d` queues that the threshold allows.
    fn min_ok(&self, d: u64) -> u64 {
        let rate = match self.cutoff {
            Some(rate) => rate,
            None => self.threshold().unwrap_or(0.0),
        };
        let need = ((rate * d as f64) - 1e-9).ceil().max(0.0) as u64;
        if self.cutoff.is_none() {
            need.max(1)
        } else {
            need
        }
    }

    /// Returns whether a setup with `ok` successes out of `d` queues should be
    /// reported. With a cutoff, this is the cutoff test. With no cutoff, only
    /// the best `limit` rates (and ties with the last of them) pass.
    pub fn keep(&self, ok: u64, d: u64) -> bool {
        match self.cutoff {
            Some(_) => ok >= self.min_ok(d),
            None => match self.threshold() {
                // Fewer than `limit` results so far, so the best of them all
                // pass.
                None => ok > 0,
                Some(rate) => ok as f64 / d as f64 + 1e-9 >= rate,
            },
        }
    }

    /// Evaluates the boards of one piece set in parallel.
    ///
    /// `solves` are the distinct solve queues of the piece set. All boards must
    /// have the same number of cells. Returns the boards that reach the
    /// threshold, with their exact success counts out of `solves.len()`.
    pub fn run_group(
        &self,
        solves: &[Vec<Piece>],
        boards: &[Board],
        saves: Saves,
        hold: bool,
    ) -> Vec<(Board, u64)> {
        let d = solves.len() as u64;
        let Some(first) = boards.first() else {
            return Vec::new();
        };
        let cells = first.0.count_ones();
        if d == 0 || cells % 4 != 0 || cells > 40 {
            return Vec::new();
        }
        let prep = Prepared::new(solves, cells, saves, hold);

        boards
            .par_iter()
            .filter_map(|&board| {
                let ok = prep.run(board, self.min_ok(d))?;
                if self.cutoff.is_none() {
                    self.record(ok as f64 / d as f64);
                }
                Some((board, ok))
            })
            .collect()
    }
}

struct Key {
    /// The first `k + 1` pieces of a solve queue. Later pieces cannot matter.
    pieces: Vec<Piece>,
    weight: u64,
}

/// Everything about one piece set that does not depend on the board.
struct Prepared {
    k: usize,
    saves: Saves,
    hold: bool,
    total: u64,
    keys: Vec<Key>,
    /// Indices of the keys that have a valid schedule, in a fixed random order.
    probe_order: Vec<u32>,
    /// Weight of the keys with no valid schedule. They fail on every board.
    dead_weight: u64,
    /// Sorted, unique order codes that the trie must solve.
    needed: Vec<u64>,
    /// Key `i` can use the orders `order_ids[offsets[i]..offsets[i + 1]]`
    /// (indices into `needed`).
    offsets: Vec<u32>,
    order_ids: Vec<u32>,
    /// Keys that failed on earlier boards. Shared by all threads. They are only
    /// a hint about which queues to test first.
    hot: [AtomicU32; HOT],
    next_hot: AtomicUsize,
}

impl Prepared {
    fn new(solves: &[Vec<Piece>], cells: u32, saves: Saves, hold: bool) -> Self {
        let k = ((40 - cells) / 4) as usize;

        let mut counts: BTreeMap<Vec<Piece>, u64> = BTreeMap::new();
        for q in solves {
            let n = q.len().min(k + 1);
            *counts.entry(q[..n].to_vec()).or_insert(0) += 1;
        }
        let keys: Vec<Key> = counts
            .into_iter()
            .map(|(pieces, weight)| Key { pieces, weight })
            .collect();

        let mut needed: Vec<u64> = Vec::new();
        for key in &keys {
            walk(&key.pieces, k, hold, 0, None, 0, 0, &mut |order, left| {
                if save_ok(saves, left, None) {
                    needed.push(order);
                }
                false
            });
        }
        needed.sort_unstable();
        needed.dedup();

        let mut offsets = vec![0u32];
        let mut order_ids: Vec<u32> = Vec::new();
        let mut probe_order: Vec<u32> = Vec::new();
        let mut dead_weight = 0;
        for (i, key) in keys.iter().enumerate() {
            let start = order_ids.len();
            walk(&key.pieces, k, hold, 0, None, 0, 0, &mut |order, left| {
                if save_ok(saves, left, None) {
                    order_ids.push(needed.binary_search(&order).unwrap() as u32);
                }
                false
            });
            if order_ids.len() == start {
                dead_weight += key.weight;
            } else {
                probe_order.push(i as u32);
            }
            offsets.push(order_ids.len() as u32);
        }

        // A fixed shuffle, so that the first queues tested are not alike.
        let mut rng = 0x9E37_79B9_7F4A_7C15u64;
        for i in (1..probe_order.len()).rev() {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            probe_order.swap(i, (rng % (i as u64 + 1)) as usize);
        }

        Self {
            k,
            saves,
            hold,
            total: solves.len() as u64,
            keys,
            probe_order,
            dead_weight,
            needed,
            offsets,
            order_ids,
            hot: std::array::from_fn(|_| AtomicU32::new(EMPTY)),
            next_hot: AtomicUsize::new(0),
        }
    }

    fn add_hot(&self, u: u32) {
        if self.hot.iter().any(|s| s.load(Relaxed) == u) {
            return;
        }
        self.hot[self.next_hot.fetch_add(1, Relaxed) % HOT].store(u, Relaxed);
    }

    /// Returns the exact success count of `board` if it is at least `min_ok`.
    /// Returns `None` if the board cannot reach `min_ok`.
    ///
    /// First, queues are tested one by one with `reachable`, the recently
    /// failed ones first. A board with more failed weight than `total -
    /// min_ok` is rejected at once. If the limit is reached with no
    /// decision, the trie solves all queues.
    fn run(&self, board: Board, min_ok: u64) -> Option<u64> {
        if min_ok > self.total {
            return None;
        }
        let budget = self.total - min_ok;
        if self.dead_weight > budget {
            return None;
        }

        let mut fails = self.dead_weight;
        let mut tested = 0usize;
        let mut hot_ids = [EMPTY; HOT];
        let mut n_hot = 0;

        for slot in &self.hot {
            let u = slot.load(Relaxed);
            if u == EMPTY {
                break;
            }
            hot_ids[n_hot] = u;
            n_hot += 1;
            tested += 1;
            let key = &self.keys[u as usize];
            if !reachable(board, &key.pieces, false, self.saves, self.hold) {
                fails += key.weight;
                if fails > budget {
                    return None;
                }
            }
        }

        let mut all_tested = true;
        for &u in &self.probe_order {
            if hot_ids[..n_hot].contains(&u) {
                continue;
            }
            if tested >= PROBE_LIMIT {
                all_tested = false;
                break;
            }
            tested += 1;
            let key = &self.keys[u as usize];
            if !reachable(board, &key.pieces, false, self.saves, self.hold) {
                fails += key.weight;
                self.add_hot(u);
                if fails > budget {
                    return None;
                }
            }
        }
        if all_tested {
            return Some(self.total - fails);
        }

        // The tests did not decide. Solve all queues with the trie.
        let mut win = vec![false; self.needed.len()];
        if self.k == 0 {
            if board == PC_4 {
                win.fill(true);
            }
        } else {
            expand(&[board.0], &self.needed, &mut win, 0, self.k);
        }

        let mut ok = 0;
        let mut learned = 0;
        for (u, key) in self.keys.iter().enumerate() {
            let ids = &self.order_ids[self.offsets[u] as usize..self.offsets[u + 1] as usize];
            if ids.iter().any(|&i| win[i as usize]) {
                ok += key.weight;
            } else if !ids.is_empty() && learned < LEARN {
                self.add_hot(u as u32);
                learned += 1;
            }
        }
        (ok >= min_ok).then_some(ok)
    }
}

/// Enumerates the ways to place `k` pieces from `queue` with the hold options
/// of `Solver::search`. It calls `f` with the order code (3 bits per piece,
/// first piece in the highest digit) and the leftover piece. If `f` returns
/// `true`, the walk stops and returns `true`.
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
        // The queue is empty. The held piece is not part of the queue, so it can
        // still be placed. This must stay in step with `Solver::search` and
        // `percent::walk`.
        //
        // The placement still adds a digit: `code` is the order of the placed
        // pieces, and `expand` reads `k` digits from it. A hold entry spends an
        // extra queue slot (option 3 below), so without this branch a solve that
        // needs every queue piece could not use hold at all.
        let Some(h) = hold else {
            return false;
        };
        return walk(queue, k, hold_on, d, None, (code << 3) | h as u64, placed + 1, f);
    };
    if walk(
        queue,
        k,
        hold_on,
        d + 1,
        hold,
        (code << 3) | active as u64,
        placed + 1,
        f,
    ) {
        return true;
    }
    if !hold_on {
        return false;
    }
    match hold {
        Some(h) => {
            h != active
                && walk(
                    queue,
                    k,
                    hold_on,
                    d + 1,
                    Some(active),
                    (code << 3) | h as u64,
                    placed + 1,
                    f,
                )
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

/// Solves the orders `codes` (sorted, unique, `k` pieces each) from `boards`
/// with no hold. `win[i]` is set for the orders that reach a perfect clear. All
/// `codes` share the prefix of length `depth`. `win` is zero at the start.
fn expand(boards: &[u64], codes: &[u64], win: &mut [bool], depth: usize, k: usize) {
    let shift = 3 * (k - 1 - depth);
    let mut lo = 0;
    while lo < codes.len() {
        let d = (codes[lo] >> shift) & 7;
        let mut hi = lo + 1;
        while hi < codes.len() && (codes[hi] >> shift) & 7 == d {
            hi += 1;
        }
        let piece = piece_from_index(d as u8);
        if depth + 1 == k {
            // Last piece. The codes are unique, so this group has one code.
            win[lo] = boards.iter().any(|&b| completes(Board(b), piece));
        } else {
            let next = children(boards, piece);
            if !next.is_empty() {
                expand(&next, &codes[lo..hi], &mut win[lo..hi], depth + 1, k);
            }
        }
        lo = hi;
    }
}

/// Returns whether placing `piece` on `board` fills the playfield. The
/// placement must fill exactly the 4 empty cells.
#[inline(always)]
fn completes(board: Board, piece: Piece) -> bool {
    let need = Board(PC_4.0 & !board.0);
    moves(board, piece).iter().any(|m| m.mask() == need)
}
