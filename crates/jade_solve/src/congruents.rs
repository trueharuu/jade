use jade_core::board::Board;
use jade_core::header::{PLAY_LINES, rows_below};
use jade_core::piece::Piece;
use jade_core::placement::Move;
use rustc_hash::FxHashSet;

use crate::solve::fast_moves;

/// The 4-row play field. Residue never leaves these rows.
const FIELD: Board = Board::new(rows_below(PLAY_LINES));
const ROW_MASK: u64 = 0x3FF;

/// One set must span the whole command. Hold lets a path use fewer queue
/// pieces than the queue holds, so one sequence can come from several queues.
pub type Paths = FxHashSet<Vec<Move>>;

const fn full_rows(b: Board) -> u32 {
    let mut n = 0;
    let mut row = 0;
    while row < 4 {
        n += ((b.0 >> (row * 10)) & ROW_MASK == ROW_MASK) as u32;
        row += 1;
    }
    n
}

/// Reports every distinct sequence of placements that builds `target` from the
/// empty field, using `queue` and hold. A sequence in `seen` is not reported
/// again.
pub fn congruents(target: Board, queue: &[Piece], seen: &mut Paths) -> Vec<Vec<Move>> {
    let mut out = walk::<true>(target, queue);
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// Returns every path, with duplicates. `LIMITS` is a const parameter so the
/// tests can run the same walk without prunes or cache.
fn walk<const LIMITS: bool>(target: Board, queue: &[Piece]) -> Vec<Vec<Move>> {
    let target = (target & FIELD).clearshift();
    let cells = target.popcount();
    if cells % 4 != 0 {
        return Vec::new(); // Every placement adds four cells.
    }
    let mut w = Walk::<LIMITS> {
        target,
        placements: cells / 4,
        full: full_rows(target),
        queue,
        dead: FxHashSet::default(),
        results: Vec::new(),
        path: Vec::with_capacity((cells / 4) as usize),
    };
    w.go(0, Board::empty(), None);
    w.results
}

struct Walk<'a, const LIMITS: bool> {
    target: Board,
    placements: u32,
    /// Full rows in `target`.
    full: u32,
    queue: &'a [Piece],
    /// States with no path to the target. A state's result depends only on
    /// (board, level, hold), so a failed state fails for every prefix.
    dead: FxHashSet<u64>,
    results: Vec<Vec<Move>>,
    path: Vec<Move>,
}

impl<const LIMITS: bool> Walk<'_, LIMITS> {
    /// Returns whether any path from this state reaches the target.
    fn go(&mut self, level: usize, b: Board, hold: Option<Piece>) -> bool {
        if b == self.target {
            self.results.push(self.path.clone());
            return true;
        }

        let mut key = 0;
        if LIMITS {
            // Each placement adds four cells, so `done` counts placements.
            // The held piece is still playable after the queue empties.
            let done = b.popcount() / 4;
            let left = self.queue.len().saturating_sub(level) + usize::from(hold.is_some());
            if done >= self.placements || (self.placements - done) as usize > left {
                return false;
            }
            // Full rows only accumulate. Targets with none are handled by the
            // subset test in `place`.
            if self.full != 0 && full_rows(b) > self.full {
                return false;
            }
            key = b.0 | hold.map_or(0, |h| (h as u64 + 1) << 40) | (level as u64) << 44;
            if self.dead.contains(&key) {
                return false;
            }
        }

        // `|=` runs both sides, so every route is walked.
        let mut hit = false;
        match self.queue.get(level) {
            // The queue is empty. Only a held piece can still be played.
            None => {
                if let Some(h) = hold {
                    hit = self.place(h, level + 1, b, None);
                }
            }
            Some(&p) => {
                hit = self.place(p, level + 1, b, hold);
                match hold {
                    None => {
                        if let Some(&n) = self.queue.get(level + 1) {
                            hit |= self.place(n, level + 2, b, Some(p));
                        }
                    }
                    // Equal pieces give the direct placement above. Skip.
                    Some(h) if h != p => hit |= self.place(h, level + 1, b, Some(p)),
                    Some(_) => {}
                }
            }
        }

        if LIMITS && !hit {
            self.dead.insert(key);
        }
        hit
    }

    #[inline]
    fn place(&mut self, piece: Piece, level: usize, b: Board, hold: Option<Piece>) -> bool {
        // A target with no full row: no cell moves and no row clears, so every
        // board must stay a subset of the target, and `clearshift` is a no-op.
        let fast = LIMITS && self.full == 0;
        let mut hit = false;
        for m in &fast_moves(piece, b) {
            let raw = b | m.mask();
            let next = if fast {
                if raw.0 & !self.target.0 != 0 {
                    continue;
                }
                raw
            } else {
                raw.clearshift()
            };
            self.path.push(m);
            hit |= self.go(level, next, hold);
            self.path.pop();
        }
        hit
    }
}