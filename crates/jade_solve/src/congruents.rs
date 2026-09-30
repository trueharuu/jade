use jade_core::board::Board;
use jade_core::header::PLAY_LINES;
use jade_core::header::rows_below;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use rustc_hash::FxHashSet;

use crate::solve::fast_moves;

/// The 4-row play field. Residue never leaves these rows.
const FIELD: Board = Board::new(rows_below(PLAY_LINES));

/// Bits in one field row.
const ROW_MASK: u64 = 0x3FF;

/// Bits in a whole field row, as a shift distance.
const ROW_SHIFT: u32 = 10;

/// The number of rows in the play field.
const ROWS: u32 = 4;

/// The number of bits in the play field, which is the top of every row.
const FIELD_BITS: u32 = ROWS * ROW_SHIFT;

/// The placement sequences already reported.
///
/// One set must span the whole command, not one queue. Hold lets a path use
/// fewer queue pieces than the queue holds, so the same sequence can be
/// reachable from more than one queue. One line per sequence needs one set.
pub type Paths = FxHashSet<Vec<Move>>;

/// Returns whether every cell of `b` can still reach a cell of `target`.
///
/// The prune rests on two facts about `clearshift`, both read from
/// `jade_core::board`:
///
/// 1. It never reorders the non-full rows. A cell in non-full row `k` of a
///    board can only end up in non-full row `k` of the target.
/// 2. A clearshifted board is a fixed point of `clearshift`. A placement can
///    only add cells to the residue, which sits directly above the full rows,
///    so raising the marker count shifts every residue row up by one together.
///
/// So with `j` full rows in `b` and `m` in `target`, a cell of `b` in row
/// `j + k` can only end up in row `m + k`. That is, `b` shifted up by `m - j`
/// rows must be a subset of the target.
///
/// A wrong assumption here would cut valid paths, so both tests must stay.
#[inline]
const fn reaches_target(b: Board, target: Board, target_markers: u32) -> bool {
    // Markers only ever accumulate, so a board cannot have more full rows than
    // its own result. This also keeps the shift below the field width.
    let markers = markers(b);
    if markers > target_markers {
        return false;
    }

    // The room test. The residue must be short enough to reach the target's
    // residue. It runs first, because the shift below drops bits that start at
    // or above `FIELD_BITS`.
    let shift = (target_markers - markers) * ROW_SHIFT;
    if b.0 >> (FIELD_BITS - shift) != 0 {
        return false;
    }

    // The subset test. Every cell must land on a cell the target holds.
    (b.0 << shift) & !target.0 == 0
}

/// The number of full rows of `b`, in the field rows.
#[inline]
const fn markers(b: Board) -> u32 {
    let mut n = 0;
    let mut row = 0;
    while row < ROWS {
        if (b.0 >> (row * ROW_SHIFT)) & ROW_MASK == ROW_MASK {
            n += 1;
        }
        row += 1;
    }
    n
}

/// Reports every distinct sequence of playable placements that builds `target`
/// from the empty field, using `queue` and hold.
///
/// Each sequence is one call to `emit`, as a slice of the walk's own buffer.
/// The callback must not hold the slice, which is reused after the call.
///
/// Hold lets a path use fewer queue pieces than the queue holds, so one
/// sequence can be reachable from more than one queue. `seen` must span every
/// queue of a command for that reason. A sequence in `seen` is not reported
/// again.
pub fn congruents<F: FnMut(&[Move])>(target: Board, queue: &[Piece], seen: &mut Paths, emit: F) {
    // Normalize like `solve` does for its start field, so a library caller and
    // the CLI agree whatever the caller passed.
    let target = (target & FIELD).clearshift();
    let cells = target.popcount();

    let mut walk = Walk {
        target,
        cells: cells as i32,
        markers: markers(target),
        queue,
        seen,
        emit,
        // A path holds one move per placement, and a path has `cells / 4` of
        // them. Reserve them once.
        path: Vec::with_capacity((cells / 4) as usize),
    };
    walk.visit(0, Board::empty(), None);
}

struct Walk<'a, F> {
    /// The goal field, clearshifted.
    target: Board,
    /// The number of cells in `target`. Every placement adds four, so a path
    /// holds `cells / 4` of them, and the goal sits at one depth only.
    cells: i32,
    /// The number of full rows in `target`, for the alignment prune.
    markers: u32,
    queue: &'a [Piece],
    seen: &'a mut Paths,
    emit: F,
    /// The placements of the current path, pushed and popped by the walk.
    path: Vec<Move>,
}

impl<F: FnMut(&[Move])> Walk<'_, F> {
    /// Walks one state. A state whose board is the target is terminal, because
    /// a path has a fixed number of placements.
    fn visit(&mut self, level: usize, b: Board, hold: Option<Piece>) {
        if b == self.target {
            if self.seen.insert(self.path.clone()) {
                (self.emit)(&self.path);
            }
            return;
        }

        // Every placement adds exactly four filled cells, so the cells still
        // needed are the gap to the target. The remaining placements can add
        // at most `4 * remaining` cells, and the held piece is still placeable
        // once the queue empties, so count it too.
        let remaining = self.queue.len() as i32 - level as i32 + i32::from(hold.is_some());
        if self.cells - b.popcount() as i32 > 4 * remaining {
            return;
        }

        // The prune. A cell of this board has to land in the target, which the
        // residue order of `clearshift` fixes in advance.
        if !reaches_target(b, self.target, self.markers) {
            return;
        }

        self.expand(level, b, hold);
    }

    /// Tries every piece choice at `level`: place the current piece, or swap
    /// with hold and place the other piece.
    ///
    /// The order is the order of `solve::Search::expand`, so the two searches
    /// report paths in the same order.
    fn expand(&mut self, i: usize, b: Board, hold: Option<Piece>) {
        // The queue is exhausted. Only a held piece can still be played.
        let Some(&piece) = self.queue.get(i) else {
            let Some(h) = hold else {
                return;
            };
            self.place_all(h, i + 1, b, None);
            return;
        };

        self.place_all(piece, i + 1, b, hold);

        match hold {
            // Swap an empty hold for the next piece, then place it.
            None => {
                if let Some(&next) = self.queue.get(i + 1) {
                    self.place_all(next, i + 2, b, Some(piece));
                }
            }
            // Swap in the held piece, then place it. If both pieces are the
            // same, the result equals the direct placement above. Skip it.
            Some(h) if h as u8 != piece as u8 => self.place_all(h, i + 1, b, Some(piece)),
            Some(_) => {}
        }
    }

    /// Tries every landed placement of `piece` on `b`. `hold` is the hold box
    /// content after the placement.
    #[inline]
    fn place_all(&mut self, piece: Piece, level: usize, b: Board, hold: Option<Piece>) {
        for m in &fast_moves(piece, b) {
            self.path.push(m);
            self.visit(level, (b | m.mask()).clearshift(), hold);
            self.path.pop();
        }
    }
}
