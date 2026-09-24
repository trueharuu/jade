use jade_core::board::Board;
use jade_core::header::PC_4;
use jade_core::header::PLAY_LINES;
use jade_core::header::rows_below;
use jade_core::piece::Piece;
use jade_nav::buffer::Moves;
use jade_nav::fast;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

/// The 4-row play field. Residue never leaves these rows.
const FIELD: Board = Board::new(rows_below(PLAY_LINES));
/// The target state: all play-field cells filled.
const PC4: Board = Board::new(PC_4);

/// Returns whether `queue` can reach the full-board state from `board` while
/// using hold.
///
/// Reaching `PC_4` at any prefix of the queue counts as success.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece]) -> bool {
    solve_inner(board, queue, true).is_some()
}

/// The level-frontier search. Returns the queue index at which the first
/// `PC_4` state is produced, or `None`. With `allow_hold`, the guideline hold
/// rule applies; without it, pieces are played strictly in queue order.
fn solve_inner(board: Board, queue: &[Piece], allow_hold: bool) -> Option<usize> {
    let board = (board & FIELD).clearshift();
    if board == PC4 {
        return Some(0);
    }

    let mut solve = Solve {
        queue,
        allow_hold,
        buckets: FxHashMap::default(),
    };

    // Record the root. The root is not `PC_4` (checked above), so ignore the
    // return value and let `walk` find the first completion.
    let _ = solve.insert(0, board, None);

    solve.walk()
}

/// One level-frontier walk. `buckets` holds the states of at most three live
/// levels: the level being expanded and its two successors.
struct Solve<'a> {
    queue: &'a [Piece],
    allow_hold: bool,
    buckets: FxHashMap<usize, FxHashSet<u64>>,
}

impl Solve<'_> {
    /// Expands every level up to the queue length, returning the level of the
    /// first `PC_4` produced.
    fn walk(&mut self) -> Option<usize> {
        let len = self.queue.len();

        for i in 0..=len {
            let Some(cur) = self.buckets.remove(&i) else {
                continue;
            };
            let Some(&piece) = self.queue.get(i) else {
                continue;
            };

            for key in cur {
                let (b, hold) = unpack(key);

                // Place the current piece.
                for m in &fast_moves(piece, b) {
                    let c = (b | m.mask()).clearshift();
                    if self.insert(i + 1, c, hold) {
                        return Some(i + 1);
                    }
                }

                if !self.allow_hold {
                    continue;
                }

                match hold {
                    // Swap an empty hold for the next piece, then place it.
                    None => {
                        let Some(&next) = self.queue.get(i + 1) else {
                            continue;
                        };
                        for m in &fast_moves(next, b) {
                            let c = (b | m.mask()).clearshift();
                            if self.insert(i + 2, c, Some(piece)) {
                                return Some(i + 2);
                            }
                        }
                    }
                    // Swap in the held piece, then place it.
                    Some(h) => {
                        for m in &fast_moves(h, b) {
                            let c = (b | m.mask()).clearshift();
                            if self.insert(i + 1, c, Some(piece)) {
                                return Some(i + 1);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Records `b` at `level`, or reports that the search is already done.
    ///
    /// Returns `true` when `b` is `PC_4`. Otherwise the state is either
    /// pruned out (and `false` returned) or recorded for later expansion.
    fn insert(&mut self, level: usize, b: Board, hold: Option<Piece>) -> bool {
        if b == PC4 {
            return true;
        }

        // Each placement adds exactly four filled cells, so a board whose
        // fill count is not a multiple of four can never reach `PC_4`.
        if !b.popcount().is_multiple_of(4) {
            return false;
        }

        // A board that can never be fully filled is dead.
        if b.has_isolated_cell() || b.has_imbalanced_split() {
            return false;
        }

        // The remaining placements can add at most `4 * level` cells.
        let remaining = self.queue.len() - level;
        if 40 - b.popcount() > 4 * remaining as u32 {
            return false;
        }

        self.buckets.entry(level).or_default().insert(pack(b, hold));
        false
    }
}

/// Generates all reachable landed placements of `piece` on `board` with the
/// fast move generator.
#[inline]
#[must_use]
fn fast_moves(piece: Piece, board: Board) -> Moves {
    match piece {
        Piece::T => fast::generate::<{ Piece::T }>(&board),
        Piece::I => fast::generate::<{ Piece::I }>(&board),
        Piece::J => fast::generate::<{ Piece::J }>(&board),
        Piece::L => fast::generate::<{ Piece::L }>(&board),
        Piece::O => fast::generate::<{ Piece::O }>(&board),
        Piece::S => fast::generate::<{ Piece::S }>(&board),
        Piece::Z => fast::generate::<{ Piece::Z }>(&board),
    }
}

/// Packs `(board, hold)` into one key: the 40 board bits plus a 3-bit hold
/// code in bits 40-42.
#[inline]
const fn pack(b: Board, hold: Option<Piece>) -> u64 {
    let code = match hold {
        None => 0,
        Some(p) => p as u64 + 1,
    };
    b.0 | (code << 40)
}

/// Reverses [`pack`].
#[inline]
const fn unpack(key: u64) -> (Board, Option<Piece>) {
    let b = Board::new(key & PC_4);
    let hold = match (key >> 40) as u8 {
        0 => None,
        n => Some(Piece::from_u8(n - 1).unwrap()),
    };
    (b, hold)
}
