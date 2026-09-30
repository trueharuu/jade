use std::cell::RefCell;

use jade_core::board::Board;
use jade_core::header::PLAY_LINES;
use jade_core::header::rows_below;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_nav::buffer::Moves;
use jade_nav::fast;

/// The 4-row play field. Residue never leaves these rows.
const FIELD: Board = Board::new(rows_below(PLAY_LINES));

/// The full 4-row field. A goal in every mode, and terminal: no piece fits on
/// it, so no transition leaves it.
const PC_4: Board = Board::lines(4);

/// The 2-row field. A goal only in two-line mode, and not terminal: the two
/// rows above it stay open, so a path may continue toward `PC_4`.
const PC_2: Board = Board::lines(2);

/// Every hold value, the empty hold included. The set that [`reachable`] and
/// [`reachable_2l`] use, so that they place no save requirement.
const SAVE_ANY: u8 = u8::MAX;

/// The index of a hold value in a save set. The empty hold is 0 and `Piece::p`
/// is `p + 1`, so a set is one bit per hold value. [`pack`] uses the same
/// numbering for the hold field of a key.
#[inline]
const fn hold_code(hold: Option<Piece>) -> u8 {
    match hold {
        None => 0,
        Some(p) => p as u8 + 1,
    }
}

/// The bit of a save set for one hold value.
#[inline]
const fn hold_bit(hold: Option<Piece>) -> u8 {
    1 << hold_code(hold)
}

/// Returns whether `hold` meets the save requirement `saves`.
#[inline]
const fn holds(saves: u8, hold: Option<Piece>) -> bool {
    saves & hold_bit(hold) != 0
}

/// The save set for a set of piece types. A save keeps a piece in hold, so the
/// empty hold never meets one and its bit stays clear. An empty `save` gives
/// the set `0`, which no hold value meets.
fn save_set(save: &[Piece]) -> u8 {
    save.iter().fold(0, |set, &p| set | hold_bit(Some(p)))
}

/// Returns whether `queue` can reach the full-board state from `board` while
/// using hold.
///
/// Reaching `PC_4` at any prefix of the queue counts as success.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece]) -> bool {
    solve_inner(board, queue, 4, SAVE_ANY)
}

/// Like [`reachable`], but a two-line PC also counts as success.
///
/// Reaching either the `PC_2` field (`Board::lines(2)`) or the full `PC_4`
/// field counts as success.
#[must_use]
pub fn reachable_2l(board: Board, queue: &[Piece]) -> bool {
    solve_inner(board, queue, 2, SAVE_ANY)
}

/// Like [`reachable`], but a goal only counts as success when the hold holds a
/// piece whose type is in `save` at that moment.
///
/// An empty hold never meets a save, because a save keeps a piece in hold. An
/// empty `save` therefore always returns `false`.
#[must_use]
pub fn reachable_save(board: Board, queue: &[Piece], save: &[Piece]) -> bool {
    solve_inner(board, queue, 4, save_set(save))
}

/// Like [`reachable_save`], but a two-line PC also counts as success.
#[must_use]
pub fn reachable_save_2l(board: Board, queue: &[Piece], save: &[Piece]) -> bool {
    solve_inner(board, queue, 2, save_set(save))
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

/// Initial table size in slots. Must be a power of two.
const TABLE_INIT: usize = 1 << 12;
/// A table larger than this is released after use, so that `reset` stays cheap.
const TABLE_KEEP: usize = 1 << 16;
/// Marks a used slot. Zero means empty, so a key of zero is still storable.
const USED: u64 = 1 << 63;
/// Fibonacci hashing constant (odd).
const HASH_MUL: u64 = 0x9E37_79B9_7F4A_7C15;

/// Open-addressing set of `u64` keys. Linear probing. Load factor <= 0.5.
///
/// A flat array has fewer cache misses than a std hash set and needs no
/// allocation after the first calls, because the table is reused.
struct Table {
    slots: Vec<u64>,
    /// `64 - log2(slots.len())`. The hash uses the high bits of the product.
    shift: u32,
    len: usize,
}

impl Table {
    fn new() -> Self {
        Self {
            slots: vec![0; TABLE_INIT],
            shift: 64 - TABLE_INIT.trailing_zeros(),
            len: 0,
        }
    }

    /// Empties the table. Keeps the memory unless it is large.
    fn reset(&mut self) {
        if self.slots.len() > TABLE_KEEP {
            *self = Self::new();
        } else {
            self.slots.fill(0);
            self.len = 0;
        }
    }

    #[inline]
    const fn index(&self, k: u64) -> usize {
        (k.wrapping_mul(HASH_MUL) >> self.shift) as usize
    }

    /// Inserts `key`. Returns `false` if it was already present.
    #[inline]
    fn insert(&mut self, key: u64) -> bool {
        if (self.len + 1) * 2 > self.slots.len() {
            self.grow();
        }
        let k = key | USED;
        let mask = self.slots.len() - 1;
        let mut i = self.index(k);
        loop {
            let s = self.slots[i];
            if s == 0 {
                self.slots[i] = k;
                self.len += 1;
                return true;
            }
            if s == k {
                return false;
            }
            i = (i + 1) & mask;
        }
    }

    #[cold]
    fn grow(&mut self) {
        let new_size = self.slots.len() * 2;
        let old = std::mem::replace(&mut self.slots, vec![0; new_size]);
        self.shift -= 1;
        let mask = self.slots.len() - 1;
        for k in old.into_iter().filter(|&k| k != 0) {
            let mut i = self.index(k);
            while self.slots[i] != 0 {
                i = (i + 1) & mask;
            }
            self.slots[i] = k;
        }
    }
}

thread_local! {
    /// One table per thread. The table is reused between calls.
    static TABLE: RefCell<Table> = RefCell::new(Table::new());
}

/// Returns whether a goal state is reachable. `lines` selects the goal set:
/// `4` accepts only `PC_4`; `2` accepts `PC_2` or `PC_4`. `saves` is the set
/// of hold values a goal state must have in hold; [`SAVE_ANY`] drops that
/// requirement. Hold is always on.
///
/// The search is a depth-first search over `(level, board, hold)`. `level` is
/// the queue index. A state is expanded at most once, so the set of expanded
/// states is the same as in a level-by-level search. The search stops at the
/// first goal.
fn solve_inner(board: Board, queue: &[Piece], lines: usize, saves: u8) -> bool {
    debug_assert!(lines == 2 || lines == 4, "lines must be 2 or 4");
    debug_assert!(queue.len() < 1 << 20, "level does not fit in the key");
    let two_l = lines == 2;

    let board = (board & FIELD).clearshift();
    // The start state has an empty hold, which only meets a save when every
    // hold value does.
    if (board == PC_4 || (two_l && board == PC_2)) && holds(saves, None) {
        return true;
    }

    // `suffix[level]` is the set of hold values that the pieces in
    // `queue[level..]` can put in hold later. It is only needed to prune, and
    // it is two entries longer than the queue because the held piece can be
    // placed after the queue ends.
    let mut suffix = Vec::new();
    if saves != SAVE_ANY {
        suffix.resize(queue.len() + 2, 0);
        for level in (0..queue.len()).rev() {
            suffix[level] = suffix[level + 1] | hold_bit(Some(queue[level]));
        }
    }

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        // Reset at the start. A panic in an earlier call cannot leave stale keys.
        table.reset();
        let mut search = Search {
            two_l,
            saves,
            suffix: &suffix,
            queue,
            table: &mut table,
        };
        search.visit(0, board, None)
    })
}

struct Search<'a> {
    /// True when the `PC_2` field is a goal alongside `PC_4`. The `PC_2` goal
    /// forces the cell bound to track the lower and full targets separately,
    /// and it disables the full-field fill heuristics.
    two_l: bool,
    /// The hold values a goal state must have in hold. `SAVE_ANY` accepts any
    /// hold, the empty one included.
    saves: u8,
    /// `suffix[level]` is the set of hold values that the pieces in
    /// `queue[level..]` can put in hold. Empty when `saves` is `SAVE_ANY`.
    suffix: &'a [u8],
    queue: &'a [Piece],
    table: &'a mut Table,
}

impl Search<'_> {
    /// Checks a candidate state. Expands it if it is new and not pruned.
    /// Returns whether a goal was reached.
    fn visit(&mut self, level: usize, b: Board, hold: Option<Piece>) -> bool {
        // The full field takes the decision on its own. No piece fits on it,
        // so no transition leaves it and the hold can never change.
        if b == PC_4 {
            return holds(self.saves, hold);
        }
        // The two-line field keeps the search open. Its two rows above stay
        // available, so a path can go on toward the full field.
        if self.two_l && b == PC_2 && holds(self.saves, hold) {
            return true;
        }

        // Cheap checks first. They reject states before the table lookup.

        // Each placement adds exactly four filled cells, so a board whose
        // fill count is not a multiple of four can never reach a goal.
        let cells = b.popcount();
        if cells & 3 != 0 {
            return false;
        }

        // A cell outside the play field never leaves the board, so only the
        // field rows may be filled.
        if b.0 & !FIELD.0 != 0 {
            return false;
        }

        // Every hold value that can appear below this state is the current one
        // or a piece from `queue[level..]`, and no other, because `expand`
        // leaves the hold alone or moves the current piece into it. When none
        // of those values meets the save, no state below can reach a goal.
        if self.saves != SAVE_ANY && (hold_bit(hold) | self.suffix[level]) & self.saves == 0 {
            return false;
        }

        // The remaining placements can add at most `4 * remaining` cells.
        // The held piece is still placeable once the queue empties, so count
        // it too.
        let remaining = self.queue.len() as i32 - level as i32 + i32::from(hold.is_some());
        let cells = cells as i32;
        if self.two_l {
            // A full-field path needs all 40 cells. A two-line path needs
            // only rows 0-1 set and empty rows above. Either bound keeps the
            // state alive.
            let full_ok = 40 - cells <= 4 * remaining;
            let low_ok = b.0 & !PC_2.0 == 0 && 20 - cells <= 4 * remaining;
            if !full_ok && !low_ok {
                return false;
            }
        } else if 40 - cells > 4 * remaining {
            return false;
        }

        // Duplicate check. Placed before the expensive checks so that each
        // distinct state pays for them once. A state that fails them stays in
        // the table as a dead state. The outcome for a state is fixed, so this
        // is safe.
        if !self.table.insert(pack(level, b, hold)) {
            return false;
        }

        // A board that can never be fully filled is dead. Only the full-board
        // search uses these checks: they reason about the whole field, and a
        // two-line path may legally leave the upper rows empty.
        if !self.two_l && (b.has_isolated_cell() || b.has_imbalanced_split()) {
            return false;
        }

        self.expand(level, b, hold)
    }

    /// Tries every piece choice at `level`: place the current piece, or swap
    /// with hold and place the other piece.
    fn expand(&mut self, i: usize, b: Board, hold: Option<Piece>) -> bool {
        // The queue is exhausted. Only a held piece can still be played.
        let Some(&piece) = self.queue.get(i) else {
            let Some(h) = hold else {
                return false;
            };
            return self.place_all(h, i + 1, b, None);
        };

        if self.place_all(piece, i + 1, b, hold) {
            return true;
        }

        match hold {
            // Swap an empty hold for the next piece, then place it.
            None => match self.queue.get(i + 1) {
                Some(&next) => self.place_all(next, i + 2, b, Some(piece)),
                None => false,
            },
            // Swap in the held piece, then place it. If both pieces are the
            // same, the result equals the direct placement above. Skip it.
            Some(h) if h as u8 != piece as u8 => self.place_all(h, i + 1, b, Some(piece)),
            Some(_) => false,
        }
    }

    /// Tries every landed placement of `piece` on `b`. `hold` is the hold box
    /// content after the placement.
    #[inline]
    fn place_all(&mut self, piece: Piece, level: usize, b: Board, hold: Option<Piece>) -> bool {
        for m in &fast_moves(piece, b) {
            if self.child(level, m, b, hold) {
                return true;
            }
        }
        false
    }

    #[inline]
    fn child(&mut self, level: usize, mv: Move, b: Board, hold: Option<Piece>) -> bool {
        self.visit(level, (b | mv.mask()).clearshift(), hold)
    }
}

/// Generates all reachable landed placements of `piece` on `board` with the
/// fast move generator.
#[inline]
#[must_use]
pub(crate) fn fast_moves(piece: Piece, board: Board) -> Moves {
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

/// Packs `(level, board, hold)` into one key: 40 board bits, a 3-bit hold code
/// in bits 40-42, and the level from bit 43. Bit 63 stays free for `USED`.
/// The caller must have checked that `b` has no cell outside the field.
#[inline]
const fn pack(level: usize, b: Board, hold: Option<Piece>) -> u64 {
    let code = hold_code(hold) as u64;
    b.0 | (code << 40) | ((level as u64) << 43)
}
