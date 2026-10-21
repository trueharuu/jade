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
    let mut raws: Vec<Vec<u16>> = paths
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
    for i in 0..n {
        buf[i] = piece_from_index(((code >> (PIECE_BITS * (n - 1 - i) as u32)) & 7) as u8);
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

    for m in &moves(board, piece) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies a sequence to `board`, returning the board after the last move.
    fn run(board: Board, seq: &[Move]) -> Board {
        let mut b = board;
        for &m in seq {
            b |= m.mask();
            b = b.clearshift();
        }
        b
    }

    #[test]
    fn an_empty_target_needs_no_moves() {
        assert_eq!(
            congruents(Board::empty(), &[Queue::new()], false),
            vec![Vec::new()]
        );
    }

    #[test]
    fn a_target_that_is_not_a_whole_number_of_pieces() {
        // Five cells cannot be built from whole pieces.
        let qs = [Queue::from_slice(&[Piece::I])];
        assert_eq!(
            congruents(Board::new(0b1_1111), &qs, false),
            [] as [std::vec::Vec<jade_core::placement::Move>; 0]
        );
    }

    #[test]
    fn a_one_piece_target() {
        // A vertical I in the left column, which is one piece.
        let one = Move::new(Piece::I, 0, 0, jade_core::rotation::Rotation::West);
        let target = run(Board::empty(), &[one]);

        let qs = [Queue::from_slice(&[Piece::I])];
        let got = congruents(target, &qs, false);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], vec![one]);
        assert_eq!(run(Board::empty(), &got[0]), target);
    }

    #[test]
    fn both_orders_of_two_pieces_are_reported() {
        // Two O pieces side by side on the floor. Either can go first.
        let a = Move::new(Piece::O, 2, 0, jade_core::rotation::Rotation::North);
        let b = Move::new(Piece::O, 4, 0, jade_core::rotation::Rotation::North);
        let target = run(Board::empty(), &[a, b]);

        let qs = [Queue::from_slice(&[Piece::O, Piece::O])];
        let got = congruents(target, &qs, false);
        assert!(got.contains(&vec![a, b]), "{got:?}");
        assert!(got.contains(&vec![b, a]), "{got:?}");

        for seq in &got {
            assert_eq!(run(Board::empty(), seq), target);
        }
    }

    #[test]
    fn a_piece_with_no_support_is_never_placed_first() {
        // The stacked O cannot be placed before the one under it, so only one
        // order is legal.
        let a = Move::new(Piece::O, 2, 0, jade_core::rotation::Rotation::North);
        let b = Move::new(Piece::O, 2, 2, jade_core::rotation::Rotation::North);
        let target = run(Board::empty(), &[a, b]);

        let qs = [Queue::from_slice(&[Piece::O, Piece::O])];
        let got = congruents(target, &qs, false);
        assert_eq!(got, vec![vec![a, b]]);
    }

    #[test]
    fn a_longer_queue_gives_the_same_answer_as_a_short_one() {
        // A memo keyed on the remaining order alone made a longer queue lose
        // the answers a shorter one found, because an earlier order marked the
        // shared states dead.
        let a = Move::new(Piece::O, 2, 0, jade_core::rotation::Rotation::North);
        let b = Move::new(Piece::O, 4, 0, jade_core::rotation::Rotation::North);
        let target = run(Board::empty(), &[a, b]);

        let short = [Queue::from_slice(&[Piece::O, Piece::O])];
        let longer = [
            Queue::from_slice(&[Piece::O, Piece::O]),
            Queue::from_slice(&[Piece::O, Piece::O, Piece::T]),
            Queue::from_slice(&[Piece::T, Piece::O, Piece::O]),
            Queue::from_slice(&[Piece::O, Piece::O, Piece::I]),
        ];

        assert_eq!(
            congruents(target, &short, true),
            congruents(target, &longer, true)
        );
    }

    #[test]
    fn a_queue_shorter_than_the_piece_count() {
        let one = Move::new(Piece::I, 0, 0, jade_core::rotation::Rotation::West);
        let target = run(Board::empty(), &[one]);

        // The target needs one piece, so an empty queue is too short.
        assert!(congruents(target, &[Queue::new()], false).is_empty());
    }

    #[test]
    fn queues_differing_past_the_cut_are_collapsed() {
        // Two O pieces on the floor. The second queue has an extra piece that
        // cannot take part, so it must give the same answer as the first.
        let a = Move::new(Piece::O, 2, 0, jade_core::rotation::Rotation::North);
        let b = Move::new(Piece::O, 4, 0, jade_core::rotation::Rotation::North);
        let target = run(Board::empty(), &[a, b]);

        let short = [Queue::from_slice(&[Piece::O, Piece::O])];
        let long = [Queue::from_slice(&[Piece::O, Piece::O, Piece::T])];

        assert_eq!(
            congruents(target, &short, false),
            congruents(target, &long, false)
        );
    }

    #[test]
    fn results_are_reported_once_across_orders() {
        // Two T pieces that stack. Both orders reach the same board, and the
        // union must not repeat a sequence.
        let a = Move::new(Piece::T, 3, 0, jade_core::rotation::Rotation::North);
        let b = Move::new(Piece::T, 3, 1, jade_core::rotation::Rotation::North);
        let target = run(Board::empty(), &[a, b]);

        let qs = [Queue::from_slice(&[Piece::T, Piece::T])];
        let got = congruents(target, &qs, false);

        let mut sorted = got.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), got.len(), "duplicate sequence in {got:?}");
    }

    #[test]
    fn hold_finds_an_order_the_queue_cannot_place_directly() {
        // An L that cannot reach the floor on its own, and a T to hold it
        // out of the way. With hold the L can be placed later in the sequence.
        let one = Move::new(Piece::I, 0, 0, jade_core::rotation::Rotation::West);
        let target = run(Board::empty(), &[one]);

        // A single I cannot be helped by holding, so the hold search must at
        // least agree with the direct one.
        let qs = [Queue::from_slice(&[Piece::I, Piece::T])];
        let with_hold = congruents(target, &qs, true);
        let without = congruents(target, &qs, false);
        assert_eq!(with_hold, without);
    }
}
