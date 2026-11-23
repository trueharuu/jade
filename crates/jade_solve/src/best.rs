//! Highest-scoring path search.
//!
//! Given a starting board and one queue, this module enumerates every
//! perfect-clear solution, scores each one, and keeps the highest-scoring
//! move sequence together with its scoring events.

use std::cell::Cell;
use std::cell::RefCell;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::score::Event;
use jade_core::score::Spin;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::moves::moves;
use crate::rules::PC_2;
use crate::rules::PC_4;
use crate::rules::outside_playfield;
use crate::rules::pieces_to_goal;
use crate::rules::pruned;
use crate::saves::Saves;
use crate::saves::save_ok;

/// The highest-scoring solution found by [`best`].
#[derive(Debug, Clone)]
pub struct BestResult {
    /// The move sequence, in placement order. Empty for score-only searches.
    pub moves: Vec<Move>,
    /// The scoring event for each move, in the same order.
    pub events: Vec<Event>,
    /// Total score of the sequence.
    pub score: u32,
}

/// Score state carried along a path.
#[derive(Clone, Copy)]
struct ScoreState {
    b2b: bool,
    combo: Option<u32>,
}

struct Ctx {
    two_l: bool,
    saves: Saves,
    suffix_codes: Vec<(u64, u8)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct StateKey {
    board: u64,
    queue_code: u64,
    queue_len: u8,
    hold: u8,
    b2b: bool,
    combo: Option<u32>,
}

#[derive(Default)]
struct Memo {
    dead: FxHashSet<StateKey>,
}

struct Best {
    found: bool,
    score: u32,
    moves: Vec<Move>,
    events: Vec<Event>,
}

thread_local! {
    static TLS_MEMO: RefCell<Memo> = RefCell::new(Memo::default());
    static TLS_GEN: Cell<u64> = const { Cell::new(0) };
}

static BATCH_GEN: AtomicU64 = AtomicU64::new(0);

/// Starts a new shared-memo batch. Call once before a batch of `best_shared`
/// calls.
pub fn best_batch_start() {
    BATCH_GEN.fetch_add(1, Ordering::SeqCst);
}

/// Returns the single highest-scoring perfect-clear path for `board` and
/// `queue`, or `None` when no solution exists.
#[must_use]
pub fn best(
    board: Board,
    queue: &[Piece],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> Option<BestResult> {
    let mut memo = Memo::default();
    best_inner::<true>(board, queue, two_l, saves, hold_enabled, &mut memo)
}

/// Score-only variant. It skips move tracking and is faster for callers that
/// only need events and score.
#[must_use]
pub fn best_score(
    board: Board,
    queue: &[Piece],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> Option<BestResult> {
    let mut memo = Memo::default();
    best_inner::<false>(board, queue, two_l, saves, hold_enabled, &mut memo)
}

/// Score-only variant that uses the thread-local shared memo of the current
/// batch.
#[must_use]
pub fn best_score_shared(
    board: Board,
    queue: &[Piece],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> Option<BestResult> {
    let generation = BATCH_GEN.load(Ordering::SeqCst);
    TLS_GEN.with(|g| {
        if g.get() != generation {
            g.set(generation);
            TLS_MEMO.with(|m| *m.borrow_mut() = Memo::default());
        }
    });
    TLS_MEMO.with(|m| {
        let mut memo = m.borrow_mut();
        best_inner::<false>(board, queue, two_l, saves, hold_enabled, &mut memo)
    })
}

#[allow(clippy::too_many_arguments)]
fn best_inner<const TRACK: bool>(
    board: Board,
    queue: &[Piece],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    memo: &mut Memo,
) -> Option<BestResult> {
    let board = board.clearshift();
    let cells = board.popcount();
    if !cells.is_multiple_of(4) || cells > 40 {
        return None;
    }

    // Precompute the code and length of the remaining queue at every depth.
    let mut suffix_codes = Vec::with_capacity(queue.len() + 1);
    for depth in 0..=queue.len() {
        let mut code = 0u64;
        for &p in &queue[depth..] {
            code = (code << 3) | p as u64;
        }
        suffix_codes.push((code, (queue.len() - depth) as u8));
    }

    let ctx = Ctx {
        two_l,
        saves,
        suffix_codes,
    };
    let mut best = Best {
        found: false,
        score: 0,
        moves: Vec::new(),
        events: Vec::new(),
    };
    let mut path: Vec<Move> = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    let mut best_acc = FxHashMap::default();

    search::<TRACK>(
        &ctx,
        memo,
        &mut best_acc,
        board,
        None,
        0,
        queue,
        hold_enabled,
        &mut path,
        &mut events,
        ScoreState {
            b2b: false,
            combo: None,
        },
        0,
        &mut best,
    );

    if !best.found {
        return None;
    }
    Some(BestResult {
        moves: best.moves,
        events: best.events,
        score: best.score,
    })
}

fn memo_key(
    ctx: &Ctx,
    board: Board,
    depth: usize,
    hold: Option<Piece>,
    state: ScoreState,
) -> StateKey {
    let (code, len) = ctx.suffix_codes[depth];
    StateKey {
        board: board.0,
        queue_code: code,
        queue_len: len,
        hold: hold.map_or(0, |p| p as u8 + 1),
        b2b: state.b2b,
        combo: state.combo,
    }
}

fn remaining_bound(cells: u32, combo: Option<u32>) -> u32 {
    if cells >= 40 {
        return 0;
    }
    let r = (40 - cells) / 4;
    if r == 0 {
        return 0;
    }
    let next_combo = u64::from(combo.map_or(0, |c| c + 1));
    let combo_sum = u64::from(r) * next_combo + (u64::from(r) * (u64::from(r) - 1)) / 2;
    let combo_score = 50 * combo_sum;
    let base_score = 3000 * u64::from(r);
    let pc_bonus = 3000u64;
    (base_score + combo_score + pc_bonus) as u32
}

#[allow(clippy::too_many_arguments)]
fn search<const TRACK: bool>(
    ctx: &Ctx,
    memo: &mut Memo,
    best_acc: &mut FxHashMap<StateKey, u32>,
    board: Board,
    hold: Option<Piece>,
    depth: usize,
    queue: &[Piece],
    hold_enabled: bool,
    path: &mut Vec<Move>,
    events: &mut Vec<Event>,
    state: ScoreState,
    acc: u32,
    best: &mut Best,
) -> bool {
    let hold = if hold_enabled { hold } else { None };

    if is_goal_board(board, ctx.two_l) && save_ok(ctx.saves, hold, queue.get(depth).copied()) {
        if !best.found || acc > best.score {
            best.found = true;
            best.score = acc;
            if TRACK {
                best.moves = path.clone();
            } else {
                best.moves.clear();
            }
            best.events = events.clone();
        }
        return true;
    }

    let cells = board.popcount();
    if cells >= 40 {
        return false;
    }
    if pruned(board) {
        return false;
    }

    let key = memo_key(ctx, board, depth, hold, state);
    if memo.dead.contains(&key) {
        return false;
    }
    if let Some(prev) = best_acc.get(&key)
        && acc <= *prev
    {
        return false;
    }

    best_acc.insert(key, acc);

    if best.found {
        let bound = remaining_bound(cells, state.combo);
        if acc + bound <= best.score {
            return false;
        }
    }

    let active = queue.get(depth).copied();
    let mut reachable = false;

    if let Some(piece) = active {
        reachable |= place::<TRACK>(
            ctx,
            memo,
            best_acc,
            board,
            cells,
            piece,
            hold,
            depth + 1,
            queue,
            hold_enabled,
            path,
            events,
            state,
            acc,
            best,
        );

        if hold_enabled {
            match hold {
                Some(h) if h != piece => {
                    reachable |= place::<TRACK>(
                        ctx,
                        memo,
                        best_acc,
                        board,
                        cells,
                        h,
                        Some(piece),
                        depth + 1,
                        queue,
                        hold_enabled,
                        path,
                        events,
                        state,
                        acc,
                        best,
                    );
                }
                None if depth + 1 < queue.len() => {
                    reachable |= place::<TRACK>(
                        ctx,
                        memo,
                        best_acc,
                        board,
                        cells,
                        queue[depth + 1],
                        Some(piece),
                        depth + 2,
                        queue,
                        hold_enabled,
                        path,
                        events,
                        state,
                        acc,
                        best,
                    );
                }
                _ => {}
            }
        }
    } else if let Some(piece) = hold {
        reachable |= place::<TRACK>(
            ctx,
            memo,
            best_acc,
            board,
            cells,
            piece,
            None,
            depth,
            queue,
            hold_enabled,
            path,
            events,
            state,
            acc,
            best,
        );
    }

    if !reachable {
        memo.dead.insert(key);
    }
    reachable
}

#[allow(clippy::too_many_arguments)]
fn place<const TRACK: bool>(
    ctx: &Ctx,
    memo: &mut Memo,
    best_acc: &mut FxHashMap<StateKey, u32>,
    board: Board,
    cells: u32,
    piece: Piece,
    hold_after: Option<Piece>,
    depth_after: usize,
    queue: &[Piece],
    hold_enabled: bool,
    path: &mut Vec<Move>,
    events: &mut Vec<Event>,
    state: ScoreState,
    acc: u32,
    best: &mut Best,
) -> bool {
    let cells_after = cells + 4;

    let k = pieces_to_goal(cells_after, ctx.two_l);
    let extra = usize::from(!ctx.saves.is_empty() && hold_after.is_none());
    let in_hold = usize::from(hold_after.is_some());
    let can_continue = queue.len().saturating_sub(depth_after) + in_hold >= k + extra;
    let goal_possible = (cells_after == 40
        && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied()))
        || (ctx.two_l
            && cells_after == 20
            && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied()));

    if !can_continue && !goal_possible {
        return false;
    }

    let mut reachable = false;
    let moves_list = moves(board, piece);

    // Collect valid candidates first so they can be tried in descending score
    // order. This improves branch-and-bound effectiveness.
    let mut candidates: Vec<(Move, Board, Event, ScoreState, u32)> = Vec::new();
    for m in &moves_list {
        let mut next = board;
        next |= m.mask();
        let lines = (next.line_clears() as u32).saturating_sub(board.line_clears() as u32);
        next = next.clearshift();

        if outside_playfield(next) {
            continue;
        }
        if pruned(next) {
            continue;
        }

        if !can_continue {
            let next_goal = is_goal_board(next, ctx.two_l);
            if !(next_goal && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied())) {
                continue;
            }
        }

        let difficult = m.spin() != Spin::None || lines == 4;
        let event_b2b = if lines > 0 {
            state.b2b && difficult
        } else {
            false
        };
        let combo_val = if lines > 0 {
            Some(state.combo.map_or(0, |c| c + 1))
        } else {
            None
        };
        let pc = is_goal_board(next, ctx.two_l);
        let event = Event {
            b2b: event_b2b,
            combo: combo_val,
            spin: m.spin(),
            lines,
            pc,
        };
        let event_score = event.score();
        let new_state = ScoreState {
            b2b: if lines > 0 { difficult } else { state.b2b },
            combo: combo_val,
        };

        candidates.push((m, next, event, new_state, event_score));
    }

    candidates.sort_by_key(|a| std::cmp::Reverse(a.4));

    for (m, next, event, new_state, event_score) in candidates {
        if TRACK {
            path.push(m);
        }
        events.push(event);
        if search::<TRACK>(
            ctx,
            memo,
            best_acc,
            next,
            hold_after,
            depth_after,
            queue,
            hold_enabled,
            path,
            events,
            new_state,
            acc + event_score,
            best,
        ) {
            reachable = true;
        }
        events.pop();
        if TRACK {
            path.pop();
        }
    }

    reachable
}

fn is_goal_board(board: Board, two_l: bool) -> bool {
    board == PC_4 || (two_l && board == PC_2)
}
