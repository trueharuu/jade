//! Inverse search: every move sequence that builds one given board.
//!
//! This is [`crate::solve`] with the goal board as a parameter instead of
//! fixed to a perfect clear. Where `solve` stops at the first success and
//! caches failures, this enumerates every success and caches reachability.
//!
//! Hold is resolved before the board search. [`crate::percent::walk`] turns
//! each queue into the piece orders its hold schedules can produce, and the
//! board search then runs over one fixed order at a time. Orders that share a
//! prefix share the work below it, which is what the suffix-keyed memo buys.

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::CAP;
use jade_core::queue::Queue;

use rustc_hash::FxHashSet;

use crate::moves::moves;
use crate::percent::walk;
use crate::rules::outside_playfield;
use crate::rules::piece_from_index;
use crate::rules::pruned;

/// Buffer for decoding a queue key. A truncated queue holds at most `CAP`
/// pieces, and `CAP` is the queue limit, so this cannot overflow.
const DECODE_BUF: usize = CAP;

/// Bits used per piece in a queue or order code.
const PIECE_BITS: u32 = 3;

/// Fixed data of one [`congruents`] call.
struct Ctx {
    target: Board,
    /// Cell count of the target. Four per placement, so it bounds the depth.
    cells: u32,
}

/// The reachability memo for one call.
///
/// The subtree below a state is the same for every path that reaches it, so a
/// state whose target is out of reach is skipped for all paths. This is the
/// one prune that does not lose paths: a state whose target *is* in reach is
/// always expanded, so every route to it is still enumerated.
///
/// The key is the board paired with the remaining piece order. Two orders
/// that agree from depth `d` onward have identical subtrees there, so they
/// share memo entries. That is what keeps many orders from costing many
/// searches.
#[derive(Default)]
struct Memo {
    /// States already proved unable to reach the target, as (order, board).
    ///
    /// `Board` carries no `Hash`, so the raw `u64` is used instead.
    dead: FxHashSet<(u64, u64)>,
}

/// Returns every move sequence that builds `target` from the empty field
/// using any of `queues`, optionally using hold.
///
/// `queue[0]` is the active piece and hold starts empty, as in
/// [`crate::solve::reachable`]. The three hold options match that function.
///
/// The search starts from [`Board::empty`], so `target` is the result rather
/// than the starting point.
///
/// `clearshift` preserves the cell count and each placement adds four, so the
/// cell count rises strictly along any path. The number of placements is
/// therefore `popcount(target) / 4` exactly, and no depth cap is needed.
///
/// Each result is a distinct sequence of moves. A `Move` records its own
/// piece, so a sequence states which piece each move placed even when hold
/// supplied it. A sequence reachable by two orders, or by two queues, is
/// reported once.
///
/// Queues are truncated to `k + 1` pieces and grouped, as in
/// [`crate::percent`]. That bound allows at most one hold: `walk` reads up to
/// index `k + h - 1`, so `h <= 1`. Schedules using more holds are out of
/// scope.
///
/// Returns an empty list when `target`'s cell count is not a multiple of four,
/// or when every queue is shorter than the placement count.
///
/// Panics if any queue is longer than [`CAP`].
#[must_use]
pub fn congruents(target: Board, queues: &[Queue], hold: bool) -> Vec<Vec<Move>> {
    let cells = target.popcount();
    if !cells.is_multiple_of(4) {
        return Vec::new();
    }

    let k = (cells / 4) as usize;
    assert!(
        !queues.iter().any(|q| q.len() > CAP),
        "queue longer than {CAP}"
    );

    // Only the first `k + 1` pieces can take part, so queues that agree there
    // cannot differ in the sequences they produce.
    let keep = k + 1;

    // Group equal truncated queues. A key is (3 bits per piece, length).
    let mut keys: Vec<(u64, u8)> = queues
        .iter()
        .map(|q| {
            let n = q.len().min(keep);
            let code = q
                .prefix(n)
                .iter()
                .fold(0u64, |c, &p| (c << PIECE_BITS) | p as u64);
            (code, n as u8)
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();

    let ctx = Ctx { target, cells };

    // Resolve hold into piece orders. Every queue contributes the orders its
    // schedules can produce, and an order that several queues share is solved
    // once.
    let mut orders: Vec<u64> = Vec::new();
    let mut buf = [Piece::T; DECODE_BUF];
    for &(code, n) in &keys {
        let n = n as usize;
        // A queue shorter than `k` cannot place enough pieces.
        if n < k {
            continue;
        }

        let q = decode(code, n, &mut buf);
        walk(q, k, hold, 0, None, 0, 0, &mut |order, _| {
            orders.push(order);
            false
        });
    }
    orders.sort_unstable();
    orders.dedup();

    let mut memo = Memo::default();
    let mut paths: Vec<Vec<Move>> = Vec::new();
    let mut path: Vec<Move> = Vec::with_capacity(k);
    let mut order_buf = [Piece::T; DECODE_BUF];

    for &order in &orders {
        let seq = decode(order, k, &mut order_buf);
        search(
            &ctx,
            &mut memo,
            Board::empty(),
            seq,
            0,
            &mut path,
            &mut paths,
        );
    }

    // A sequence can be produced by more than one order, for instance when two
    // orders differ only in a piece that was held rather than placed. Report
    // each distinct sequence once.
    //
    // `Move` has no `Ord`, so the sequences are compared as their raw words.
    let mut raws: Vec<Vec<u32>> = paths
        .into_iter()
        .map(|seq| seq.iter().map(|m| m.raw()).collect())
        .collect();
    raws.sort_unstable();
    raws.dedup();

    raws.into_iter()
        .map(|raw| {
            raw.into_iter()
                // The words come from `Move` values this call built, so each
                // is a valid move.
                .map(|w| unsafe { Move::from_raw(w) })
                .collect()
        })
        .collect()
}

/// Decodes a queue or order key into pieces.
///
/// A key of all `T` bits decodes the same at several lengths, so the length
/// is the caller's to keep. The caller must pass the same `n` it packed with.
fn decode(code: u64, n: usize, buf: &mut [Piece; DECODE_BUF]) -> &[Piece] {
    for (i, x) in buf.iter_mut().take(n).enumerate() {
        *x = piece_from_index(((code >> (PIECE_BITS * (n - 1 - i) as u32)) & 7) as u8);
    }
    &buf[..n]
}

/// The memo key for a state: the whole piece order, plus the depth and the
/// board.
///
/// A suffix is not enough. Two orders can agree on the pieces still to come
/// and reach different boards, because their prefixes differ, so the subtree
/// below them is not shared. Keying on the suffix alone makes order `O,O`
/// inherit the dead marks of order `O,T` and lose its own solutions.
///
/// The depth is redundant given the suffix length, but it keeps the packing
/// unambiguous when the remainder is empty and every piece is `T`. The order
/// carries a leading 1 bit for the same reason.
fn key(board: Board, seq: &[Piece], d: usize) -> (u64, u64) {
    let mut code = 1u64;
    for &p in seq {
        code = (code << PIECE_BITS) | p as u64;
    }

    (code ^ ((d as u64) << 40), board.0)
}

/// Extends `path` from `board`, placing `seq` in order and collecting every
/// sequence that reaches the target.
///
/// Every branch is walked. Stopping at the first solution would lose the
/// alternative move orders that reach the same target, so the only prune is
/// the memo: a state with no solution below it cannot be on one.
fn search(
    ctx: &Ctx,
    memo: &mut Memo,
    board: Board,
    seq: &[Piece],
    d: usize,
    path: &mut Vec<Move>,
    out: &mut Vec<Vec<Move>>,
) {
    if board == ctx.target {
        out.push(path.clone());
        return;
    }

    // Cells only ever rise, so a board at the target's count that is not the
    // target cannot be extended into it.
    if board.popcount() >= ctx.cells {
        return;
    }

    if pruned(board) {
        return;
    }

    let Some((&piece, rest)) = seq.split_first() else {
        return;
    };

    let k = key(board, seq, d);
    if memo.dead.contains(&k) {
        return;
    }

    let before = out.len();
    let before_depth = path.len();

    for m in &moves::<false>(board, piece) {
        let mut next = board;
        next |= m.mask();
        next = next.clearshift();

        if outside_playfield(next) {
            continue;
        }

        path.push(m);
        search(ctx, memo, next, rest, d + 1, path, out);
        path.pop();
    }

    debug_assert_eq!(path.len(), before_depth);

    // Only a state with no solution below it is worth remembering. A state
    // with one must stay expanded, or the alternative paths to it would be
    // lost, so it is never inserted.
    if out.len() == before {
        memo.dead.insert(k);
    }
}
