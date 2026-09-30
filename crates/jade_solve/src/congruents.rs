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
/// 1. It never reorders the non-full rows, and it writes them above the full
///    rows. So a cell's row index is the count of full rows plus its own index
///    among the non-full rows.
/// 2. A clearshifted board is a fixed point of `clearshift`. A placement can
///    only add cells to the residue.
///
/// A cell whose row stays non-full keeps its row index, because every new full
/// row takes one off its index among the non-full rows and adds one to the full
/// row count. So when the target has no full row, no state has one either, and
/// no cell ever moves. The board must then already be a subset of the target.
///
/// That test needs a target with no full row. A target with one gives no sound
/// per-cell test, because a cell can also end in a full row, and the full rows
/// of a target hold a cell at every column. Those targets get the full row
/// count test alone.
///
/// A wrong assumption here would cut valid paths, so the test must stay.
#[inline]
const fn reaches_target(b: Board, target: Board, target_markers: u32) -> bool {
    // Full rows only ever accumulate, so a board cannot have more of them than
    // its own result.
    if markers(b) > target_markers {
        return false;
    }
    if target_markers == 0 {
        return b.0 & !target.0 == 0;
    }
    true
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

    let mut walk = Walk::<F, true> {
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

/// A depth-first walk of the tree, one route per node, with no state table.
///
/// A path is a prefix plus a continuation, so two routes that reach the same
/// state give two different paths. The table in `solve` keeps the first route
/// to each state and drops the rest, so it cannot be used here.
///
/// `LIMITS` is a const parameter, not a field, so the prunes cost nothing when
/// they are off. The tests walk without them, to check that the prunes keep
/// every path.
struct Walk<'a, F, const LIMITS: bool> {
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

impl<F: FnMut(&[Move]), const LIMITS: bool> Walk<'_, F, LIMITS> {
    /// Walks one state. A state whose board is the target is terminal, because
    /// a path has a fixed number of placements.
    fn visit(&mut self, level: usize, b: Board, hold: Option<Piece>) {
        if b == self.target {
            if self.seen.insert(self.path.clone()) {
                (self.emit)(&self.path);
            }
            return;
        }

        if LIMITS {
            // Every placement adds exactly four filled cells, so the cells still
            // needed are the gap to the target. The remaining placements can add
            // at most `4 * remaining` cells, and the held piece is still
            // placeable once the queue empties, so count it too.
            let remaining = self.queue.len() as i32 - level as i32 + i32::from(hold.is_some());
            if self.cells - b.popcount() as i32 > 4 * remaining {
                return;
            }

            // The prune. A cell of this board has to land in the target, and the
            // residue order of `clearshift` fixes where in advance.
            if !reaches_target(b, self.target, self.markers) {
                return;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The queues the differential test runs, chosen so the walk has to use
    /// hold, and so a target that one queue can build another cannot.
    const QUEUES: [[Piece; 3]; 4] = [
        [Piece::I, Piece::J, Piece::L],
        [Piece::T, Piece::T, Piece::T],
        [Piece::O, Piece::O, Piece::O],
        [Piece::S, Piece::Z, Piece::I],
    ];

    /// A T lock the move generator can reach, used as a four-cell target. It
    /// comes from the generator, so the test does not depend on which T shapes
    /// are reachable from the spawn.
    fn reachable_t() -> Move {
        fast_moves(Piece::T, Board::empty()).iter().next().unwrap()
    }

    /// Every path that builds `target` with `queue`, sorted.
    ///
    /// The two walks differ only in the const parameter, so one function runs
    /// both the pruned walk and the reference walk.
    fn paths<const LIMITS: bool>(target: Board, queue: &[Piece]) -> Vec<Vec<Move>> {
        let target = (target & FIELD).clearshift();
        let cells = target.popcount();
        let mut seen = Paths::default();
        let mut reported: Vec<Vec<Move>> = Vec::new();

        let mut walk = Walk::<_, LIMITS> {
            target,
            cells: cells as i32,
            markers: markers(target),
            queue,
            seen: &mut seen,
            emit: |p: &[Move]| reported.push(p.to_vec()),
            path: Vec::new(),
        };
        walk.visit(0, Board::empty(), None);
        drop(walk);

        // The set and the reports must hold the same paths, whichever walk ran.
        let mut set: Vec<Vec<Move>> = seen.into_iter().collect();
        sort_paths(&mut set);
        sort_paths(&mut reported);
        assert_eq!(reported, set, "the set and the reports disagree");
        reported
    }

    /// Sorts paths by the raw value of each move, so two path lists can be
    /// compared. `Move` has no order, so its packed form stands in for one.
    fn sort_paths(paths: &mut [Vec<Move>]) {
        paths.sort_by(|a, b| a.iter().map(|m| m.raw()).cmp(b.iter().map(|m| m.raw())));
    }

    /// The most filled cells in any field row of `b`.
    const fn fullest_row(b: Board) -> u32 {
        let mut best = 0;
        let mut row = 0;
        while row < ROWS {
            let n = ((b.0 >> (row * ROW_SHIFT)) & ROW_MASK).count_ones();
            if n > best {
                best = n;
            }
            row += 1;
        }
        best
    }

    /// Boards that `depth` placements from the empty field can reach, sorted by
    /// their raw bits and cut to `limit` entries, so a test is deterministic
    /// and bounded. When `full_rows` is set, only the boards with at least
    /// one full row are kept.
    ///
    /// The pieces are the three that can tile a full row, which is the only way
    /// to reach a board with one inside three placements.
    fn boards_within(depth: u32, full_rows: bool, limit: usize) -> Vec<Board> {
        const PIECES: [Piece; 3] = [Piece::I, Piece::J, Piece::L];

        let mut found = FxHashSet::default();
        let mut frontier = vec![Board::empty()];
        for _ in 0..depth {
            let mut next = Vec::new();
            for b in &frontier {
                for piece in PIECES {
                    for m in &fast_moves(piece, *b) {
                        next.push((*b | m.mask()).clearshift());
                    }
                }
            }
            // `Board` has no hash, so the set holds the raw bits.
            found.extend(
                next.iter()
                    .filter(|b| !full_rows || markers(**b) > 0)
                    .map(|b| b.0),
            );

            // A full row holds ten cells and each placement adds four, so a
            // board whose fullest row is under `10 - 4 * left` cannot reach one
            // in the placements that are left. The walk is much smaller without
            // this.
            let left = depth - level_of(&next);
            let need = 10u32.saturating_sub(4 * left);
            frontier = next
                .into_iter()
                .filter(|b| fullest_row(*b) >= need)
                .collect();
        }

        let mut all: Vec<Board> = found.into_iter().map(Board::new).collect();
        all.sort_by_key(|b| b.0);
        all.truncate(limit);
        all
    }

    /// The number of placements made in the boards of `next`. Every board there
    /// holds the same count, so the cell count gives it.
    fn level_of(next: &[Board]) -> u32 {
        let cells = next.first().map_or(0, |b| b.popcount());
        debug_assert!(next.iter().all(|b| b.popcount() == cells));
        debug_assert!(cells.is_multiple_of(4));
        cells / 4
    }

    #[test]
    fn one_path_for_a_single_piece() {
        let mv = reachable_t();

        let mut seen = Paths::default();
        let mut reported: Vec<Vec<Move>> = Vec::new();
        congruents(mv.mask(), &[Piece::T], &mut seen, |p| {
            reported.push(p.to_vec());
        });

        assert_eq!(reported, vec![vec![mv]]);
    }

    #[test]
    fn no_path_when_the_piece_cannot_cover_the_target() {
        let target = reachable_t().mask();

        let mut seen = Paths::default();
        let mut reported: Vec<Vec<Move>> = Vec::new();
        congruents(target, &[Piece::I], &mut seen, |p| {
            reported.push(p.to_vec());
        });

        assert!(
            reported.is_empty(),
            "an I cannot cover a T shape: {reported:?}"
        );
    }

    #[test]
    fn one_empty_path_for_an_empty_target() {
        let mut seen = Paths::default();
        let mut reported: Vec<Vec<Move>> = Vec::new();
        congruents(Board::empty(), &[Piece::T], &mut seen, |p| {
            reported.push(p.to_vec());
        });

        assert_eq!(reported, vec![Vec::new()]);
    }

    #[test]
    fn one_set_covers_every_queue() {
        // The target needs one placement, and each queue below can place that
        // `T` as its first piece, so both find the same path. The set spans the
        // command, so the path is reported once.
        let mv = reachable_t();

        let mut seen = Paths::default();
        let mut reported: Vec<Vec<Move>> = Vec::new();
        for queue in [[Piece::T, Piece::O], [Piece::T, Piece::I]] {
            congruents(mv.mask(), &queue, &mut seen, |p| reported.push(p.to_vec()));
        }

        assert_eq!(reported, vec![vec![mv]]);
    }

    #[test]
    fn the_prunes_keep_every_path() {
        // Two placements is eight cells, which the reference walk can afford.
        let targets = boards_within(2, false, 20);
        assert_eq!(targets.len(), 20, "the target list is not full");

        for target in targets {
            for queue in QUEUES {
                assert_eq!(
                    paths::<true>(target, &queue),
                    paths::<false>(target, &queue),
                    "target {:#x} queue {queue:?}",
                    target.0
                );
            }
        }
    }

    #[test]
    fn the_prunes_keep_every_path_into_a_full_row() {
        // A full row holds ten cells, so the smallest target with one takes
        // three placements. The reference walk can afford three, and no more.
        let candidates = boards_within(3, true, 8);
        assert!(
            !candidates.is_empty(),
            "no three-placement board has a full row"
        );

        // Check every target the reference walk can build, so the test covers
        // paths that end in a full row and not only the empty answer. The
        // reference walk costs about a million states per target, so the list
        // is cut.
        let builder: [Piece; 3] = [Piece::I, Piece::J, Piece::L];
        let mut checked = 0;
        for target in candidates {
            if paths::<false>(target, &builder).is_empty() {
                continue;
            }
            assert!(markers(target) > 0, "the target has no full row");

            for queue in [builder, [Piece::T, Piece::T, Piece::T]] {
                assert_eq!(
                    paths::<true>(target, &queue),
                    paths::<false>(target, &queue),
                    "target {:#x} queue {queue:?}",
                    target.0
                );
            }
            checked += 1;
            if checked == 3 {
                break;
            }
        }
        assert!(checked > 0, "no full-row target is buildable with I J L");
    }
}
