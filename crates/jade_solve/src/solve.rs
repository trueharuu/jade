use std::cell::RefCell;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::queue::CAP;

use crate::rules::completes_goal;
use crate::rules::outside_playfield;
use crate::rules::pieces_to_goal;
use crate::rules::pruned;
use crate::saves::Saves;
use crate::saves::save_ok;

/// Failure cache size is `2^CACHE_BITS` words (128 KB at 14 bits).
const CACHE_BITS: u32 = 14;
const CACHE_LEN: usize = 1 << CACHE_BITS;
/// A `Node` uses 48 bits. The upper 16 bits of a cache word hold the generation.
const KEY_MASK: u64 = (1 << 48) - 1;
const HASH_MUL: u64 = 0x9E37_79B9_7F4A_7C15;

// These keep the paths that existed before the module split working for
// callers outside the crate.
pub use crate::moves::moves;
pub use crate::rules::PC_2;
pub use crate::rules::PC_4;

/// Fixed data of one `reachable` call.
struct Ctx<'a> {
    queue: &'a [Piece],
    two_l: bool,
    saves: Saves,
    hold: bool,
    /// Generation of this call, shifted into the upper 16 bits.
    stamp: u64,
}

/// DFS solver. It owns a lossy cache of failed nodes. Use one per thread.
///
/// The search stops at the first success. Every cached node is therefore a
/// failed node, and no value is stored. A lost cache entry only costs time.
pub struct Solver {
    table: Box<[u64; CACHE_LEN]>,
    generation: u16,
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver {
    pub fn new() -> Self {
        Self {
            // The `try_into` cannot fail: the length is `CACHE_LEN`.
            table: vec![0u64; CACHE_LEN]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            generation: 0,
        }
    }

    /// Returns a new generation stamp. Generation 0 is never used: an empty
    /// slot (0) must not match any word. On wrap, the table is cleared.
    fn next_stamp(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.table.fill(0);
            self.generation = 1;
        }
        (self.generation as u64) << 48
    }

    /// Returns `true` if `word` is already cached. Otherwise stores it.
    #[inline(always)]
    fn seen(&mut self, word: u64) -> bool {
        // The index is below `CACHE_LEN` by construction (top `CACHE_BITS` bits).
        let i = ((word & KEY_MASK).wrapping_mul(HASH_MUL) >> (64 - CACHE_BITS)) as usize;
        let slot = &mut self.table[i];
        if *slot == word {
            true
        } else {
            *slot = word;
            false
        }
    }

    /// See [`reachable`].
    #[must_use]
    pub fn reachable(
        &mut self,
        board: Board,
        queue: &[Piece],
        two_l: bool,
        saves: Saves,
        hold: bool,
    ) -> bool {
        assert!(queue.len() <= CAP);
        // A full row above an empty cell is not a state the search can reach.
        // The caller must pass a board that `clearshift` does not change.
        debug_assert!(board.clearshift() == board, "root board is not clearshifted");

        let cells = board.0.count_ones();
        if cells % 4 != 0 {
            return false;
        }

        let root = Node::new(board, None, 0);
        if root.is_goal(two_l, saves, queue.first().copied()) {
            return true;
        }
        if cells >= 40 || root.pruned() {
            return false;
        }

        // Count bound. Hold is empty at the root, so a non-empty `saves` needs
        // one extra piece as the leftover.
        let k = pieces_to_goal(cells, two_l);
        if queue.len() < k + usize::from(!saves.is_empty()) {
            return false;
        }

        // Without hold, depth equals the number of placed pieces. A goal can
        // then occur only at fixed depths, and the leftover is `queue[depth]`.
        if !hold && !saves.is_empty() {
            let target_ok = |target: u32| {
                target >= cells
                    && queue
                        .get(((target - cells) / 4) as usize)
                        .is_some_and(|&p| saves.has(p))
            };
            if !(target_ok(40) || (two_l && target_ok(20))) {
                return false;
            }
        }

        let ctx = Ctx {
            queue,
            two_l,
            saves,
            hold,
            stamp: self.next_stamp(),
        };
        self.search(&ctx, &root)
    }

    /// Expands `node`. The node is not a goal, is not pruned, and passed the
    /// count bound.
    ///
    /// The three hold options below must stay in step with `percent::walk`.
    /// That function encodes the same rules without a board. The two are not
    /// merged because this one carries a board and a failure cache and that one
    /// carries an order code.
    fn search(&mut self, ctx: &Ctx, node: &Node) -> bool {
        let d = node.depth() as usize;
        let Some(&active) = ctx.queue.get(d) else {
            // The queue is empty. The held piece is not part of the queue, so
            // it can still be placed. A hold entry spends an extra queue slot
            // (option 3 below), so without this branch a solve that needs every
            // queue piece cannot use hold at all.
            let Some(h) = node.hold() else {
                return false;
            };
            let board = node.board();
            let cells = board.0.count_ones();
            return self.place(ctx, board, cells, h, None, d);
        };
        let board = node.board();
        let hold = node.hold();
        let cells = board.0.count_ones();

        // 1. Place the active piece.
        if self.place(ctx, board, cells, active, hold, d + 1) {
            return true;
        }
        if !ctx.hold {
            return false;
        }
        match hold {
            // 2. Place the hold piece. If it equals the active piece, option 1
            // already covered the same states.
            Some(h) => h != active && self.place(ctx, board, cells, h, Some(active), d + 1),
            // 3. Hold the active piece, then place the next piece.
            None => {
                d + 1 < ctx.queue.len()
                    && self.place(ctx, board, cells, ctx.queue[d + 1], Some(active), d + 2)
            }
        }
    }

    /// Places `piece` in every legal way. `hold` and `depth` are the values
    /// after the placement. Returns whether a goal is reachable.
    #[inline(always)]
    fn place(
        &mut self,
        ctx: &Ctx,
        board: Board,
        cells: u32,
        piece: Piece,
        hold: Option<Piece>,
        depth: usize,
    ) -> bool {
        let cells = cells + 4;
        let active = ctx.queue.get(depth).copied();

        // The last placement must fill exactly the 4 empty cells. A legal
        // placement elsewhere (for example above the playfield) does not count.
        if cells == 40 {
            if !save_ok(ctx.saves, hold, active) {
                return false;
            }
            return completes_goal(board, piece, PC_4);
        }

        // All children share these values. They are checked before `moves`,
        // which is the main cost.
        let goal20 = ctx.two_l && cells == 20 && save_ok(ctx.saves, hold, active);
        let k = pieces_to_goal(cells, ctx.two_l);
        // With empty hold and a save set, one more piece must remain as leftover.
        let extra = usize::from(!ctx.saves.is_empty() && hold.is_none());
        // The held piece is not part of the queue, so it is one more piece that
        // can still be placed. Counting only queue slots here rejects solves
        // that need every queue piece and use hold.
        let in_hold = usize::from(hold.is_some());
        let can_continue = ctx.queue.len() - depth + in_hold >= k + extra;
        if !can_continue && !goal20 {
            return false;
        }

        for m in &moves(board, piece) {
            let mut next = board;
            next |= m.mask();
            next = next.clearshift();

            if goal20 && next == PC_2 {
                return true;
            }
            if !can_continue {
                continue;
            }

            // Bits at or above bit 40 would overwrite the hold and depth fields.
            if outside_playfield(next) {
                continue;
            }

            let child = Node::new(next, hold, depth as u8);
            if child.pruned() {
                continue;
            }
            if self.seen(child.0 | ctx.stamp) {
                continue;
            }
            if self.search(ctx, &child) {
                return true;
            }
        }
        false
    }
}

thread_local! {
    static SOLVER: RefCell<Solver> = RefCell::new(Solver::new());
}

/// Performs DFS to determine if `queue` can reach the goal from `board` while
/// optionally using hold (if available).
///
/// `queue[0]` is the active piece and hold starts empty. With `hold`, a
/// placement uses the active piece, the hold piece, or (if hold is empty) holds
/// the active piece and places the next one.
///
/// Reaching the goal at any prefix of the queue counts as success.
///
/// When `saves` is nonempty, the leftover piece when attaining `goal` must be
/// within that set. The leftover is the hold piece, or the active piece if hold
/// is empty. An empty `saves` is no restriction.
///
/// A goal is defined to be exactly [`PC_4`] or either [`PC_2`] or [`PC_4`] if
/// `two_l` is true.
///
/// Returns `false` if the cell count of `board` is not a multiple of 4.
/// Panics if `queue` is longer than [`CAP`].
///
/// This uses a thread-local [`Solver`]. Own a `Solver` to avoid the lookup.
#[must_use]
pub fn reachable(board: Board, queue: &[Piece], two_l: bool, saves: Saves, hold: bool) -> bool {
    SOLVER.with_borrow_mut(|s| s.reachable(board, queue, two_l, saves, hold))
}

/// A thin wrapper that stores a `Board`, an optional hold `Piece`, and the
/// depth among the tree this node inhabits.
///
/// Layout: board bits 0..40, hold bits 40..44 (0 = empty, else piece + 1),
/// depth bits 44..48. Bits 48..64 are zero.
#[repr(transparent)]
pub struct Node(u64);

impl Node {
    pub const fn new(board: Board, hold: Option<Piece>, depth: u8) -> Self {
        let hold = match hold {
            Some(p) => p as u64 + 1,
            None => 0,
        };
        let depth = depth as u64;
        Node(board.0 | (hold << 40) | (depth << 44))
    }

    pub const fn board(&self) -> Board {
        Board(self.0 & ((1 << 40) - 1))
    }

    pub const fn hold(&self) -> Option<Piece> {
        let hold = (self.0 >> 40) & 0xf;
        if hold == 0 {
            None
        } else {
            // Valid by the layout assertion at the top of this file.
            Some(unsafe { std::mem::transmute((hold - 1) as u8) })
        }
    }

    pub const fn depth(&self) -> u8 {
        ((self.0 >> 44) & 0xf) as u8
    }

    /// Returns whether the board can never reach any goal.
    #[inline]
    #[must_use]
    pub const fn pruned(&self) -> bool {
        pruned(self.board())
    }

    /// Returns whether the board is `PC_2` or `PC_4` as `two_l` requires, and
    /// the leftover satisfies `save`.
    pub fn is_goal(&self, two_l: bool, save: Saves, active: Option<Piece>) -> bool {
        let board = self.board();
        let is_pc = if two_l {
            board == PC_2 || board == PC_4
        } else {
            board == PC_4
        };
        is_pc && save_ok(save, self.hold(), active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_round_trip() {
        let board = Board(0xAB_CDEF_0123);
        for depth in 0..=15u8 {
            let holds = std::iter::once(None).chain(Piece::ALL.iter().map(|&p| Some(p)));
            for hold in holds {
                let n = Node::new(board, hold, depth);
                assert_eq!(n.board(), board);
                assert_eq!(n.hold(), hold);
                assert_eq!(n.depth(), depth);
            }
        }
    }
}