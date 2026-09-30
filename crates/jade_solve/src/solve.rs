use jade_core::board::Board;
use jade_core::header::PC_4;
use jade_core::header::PLAY_LINES;
use jade_core::header::rows_below;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_nav::buffer::Moves;
use jade_nav::fast;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

/// The 4-row play field. Residue never leaves these rows.
const FIELD: Board = Board::new(rows_below(PLAY_LINES));

/// Returns whether `queue` can reach the full-board state from `board` while
/// using hold.
///
/// Reaching `PC_4` at any prefix of the queue counts as success.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece]) -> bool {
    solve_inner(board, queue, 4, true)
}

/// Like [`reachable`], but a two-line PC also counts as success.
///
/// Reaching either the `PC_2` field (`Board::lines(2)`) or the full `PC_4`
/// field counts as success.
#[must_use]
pub fn reachable_2l(board: Board, queue: &[Piece]) -> bool {
    solve_inner(board, queue, 2, true)
}

/// Returns whether `board` can never be filled.
///
/// A board fails when it has an isolated empty cell or an imbalanced split.
/// Both checks reason about the whole 4-row field, so a two-line search may
/// still succeed on a board that fails. Use this to reject a field before a
/// search when only a full-board answer matters.
#[inline]
#[must_use]
pub const fn is_unfillable(board: Board) -> bool {
    board.has_isolated_cell() || board.has_imbalanced_split()
}

/// The level-frontier search. Returns whether a goal state was reached.
/// `lines` selects the goal set: `4` accepts only `PC_4`; `2` accepts `PC_2`
/// or `PC_4`. With `allow_hold`, the guideline hold rule applies; without it,
/// pieces are played strictly in queue order.
///
/// The search is single-threaded. A level is expanded in full before the next
/// level starts, and each level is deduplicated through a hash set, so the
/// expanded state count is the number of distinct `(board, hold)` pairs.
fn solve_inner(board: Board, queue: &[Piece], lines: usize, allow_hold: bool) -> bool {
    let goals = match lines {
        2 => [Board::lines(2), Board::lines(4)],
        4 => [Board::lines(4), Board::lines(4)],
        _ => unreachable!("lines must be 2 or 4"),
    };
    let n_goals = if lines == 2 { 2 } else { 1 };
    let board = (board & FIELD).clearshift();
    if goals[..n_goals].contains(&board) {
        return true;
    }

    let mut solve = Solve {
        goals,
        n_goals,
        two_l: lines == 2,
        queue,
        allow_hold,
        buckets: FxHashMap::default(),
    };

    // Record the root. The root is not the target (checked above), so ignore
    // the return value and let `walk` find the first completion.
    let _ = solve.insert(0, board, None);

    solve.walk()
}

/// One level-frontier walk. `buckets` holds the states of at most three live
/// levels: the level being expanded and its two successors.
struct Solve<'a> {
    /// Terminal states accepted as success. Only the first `n_goals` entries
    /// are live.
    goals: [Board; 2],
    n_goals: usize,
    /// True when the `PC_2` goal is present alongside `PC_4`. The `PC_2` goal
    /// forces the time-bound to track the lower and full targets separately,
    /// and it disables the full-field fill heuristics.
    two_l: bool,
    queue: &'a [Piece],
    allow_hold: bool,
    buckets: FxHashMap<usize, FxHashSet<u64>>,
}

impl Solve<'_> {
    /// Expands every level up to the queue length, returning whether a target
    /// state was reached.
    fn walk(&mut self) -> bool {
        let len = self.queue.len();

        for i in 0..=len {
            let Some(cur) = self.buckets.remove(&i) else {
                continue;
            };

            // The queue is exhausted. Only a held piece can still be played.
            if i == len {
                if !self.allow_hold {
                    continue;
                }
                for key in cur {
                    let (b, hold) = unpack(key);
                    let Some(h) = hold else {
                        continue;
                    };
                    for m in &fast_moves(h, b) {
                        if self.play(i + 1, m, b, None) {
                            return true;
                        }
                    }
                }
                continue;
            }

            let Some(&piece) = self.queue.get(i) else {
                continue;
            };

            for key in cur {
                let (b, hold) = unpack(key);

                // Place the current piece.
                for m in &fast_moves(piece, b) {
                    if self.play(i + 1, m, b, hold) {
                        return true;
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
                            if self.play(i + 2, m, b, Some(piece)) {
                                return true;
                            }
                        }
                    }
                    // Swap in the held piece, then place it.
                    Some(h) => {
                        for m in &fast_moves(h, b) {
                            if self.play(i + 1, m, b, Some(piece)) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Places the piece for `mv` on `b` and records the child at `level`.
    /// Returns whether the child is the target field.
    ///
    /// `piece_hold` is the hold box content after the placement.
    fn play(
        &mut self,
        level: usize,
        mv: Move,
        b: Board,
        piece_hold: Option<Piece>,
    ) -> bool {
        let c = (b | mv.mask()).clearshift();
        self.insert(level, c, piece_hold)
    }

    /// Records `b` at `level`, or reports that `b` is a goal field.
    ///
    /// Returns `true` when `b` is one of the goal fields. Otherwise the state
    /// is either pruned from the search or recorded for later expansion.
    fn insert(&mut self, level: usize, b: Board, hold: Option<Piece>) -> bool {
        if self.goals[..self.n_goals].contains(&b) {
            return true;
        }

        // Each placement adds exactly four filled cells, so a board whose
        // fill count is not a multiple of four can never reach a goal.
        if !b.popcount().is_multiple_of(4) {
            return false;
        }

        // A cell outside the play field never leaves the board, so only the
        // field rows may be filled.
        if b.0 & !FIELD.0 != 0 {
            return false;
        }

        // A board that can never be fully filled is dead. Only the full-board
        // search uses these checks: they reason about the whole field, and a
        // two-line path may legally leave the upper rows empty.
        if !self.two_l && (b.has_isolated_cell() || b.has_imbalanced_split()) {
            return false;
        }

        // The remaining placements can add at most `4 * remaining` cells.
        // The held piece is still placeable once the queue empties, so count
        // it too.
        let remaining = self.queue.len() as i32 - level as i32 + i32::from(hold.is_some());
        let cells = b.popcount() as i32;
        if self.two_l {
            // A full-field path needs all 40 cells. A two-line path needs
            // only rows 0-1 set and empty rows above. Either bound keeps the
            // state alive.
            let full_ok = 40 - cells <= 4 * remaining;
            let low_ok = b.0 & !self.goals[0].0 == 0 && 20 - cells <= 4 * remaining;
            if !full_ok && !low_ok {
                return false;
            }
        } else if 40 - cells > 4 * remaining {
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
