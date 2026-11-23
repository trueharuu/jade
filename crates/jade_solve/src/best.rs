//! Highest-scoring path search.
//!
//! Given a starting board and one queue, this module enumerates every
//! perfect-clear solution, scores each one, and keeps the highest-scoring
//! move sequence together with its scoring events.

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::score::Event;
use jade_core::score::Spin;

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
    /// The move sequence, in placement order.
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
}

#[derive(Default)]
struct Memo {
    dead: FxHashSet<(u64, u64)>,
}

struct Best {
    found: bool,
    score: u32,
    moves: Vec<Move>,
    events: Vec<Event>,
}

const MAX_EVENT_SCORE: u32 = 10000;

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
    let board = board.clearshift();
    let cells = board.popcount();
    if !cells.is_multiple_of(4) || cells > 40 {
        return None;
    }

    let ctx = Ctx { two_l, saves };
    let mut memo = Memo::default();
    let mut best = Best {
        found: false,
        score: 0,
        moves: Vec::new(),
        events: Vec::new(),
    };
    let mut path: Vec<Move> = Vec::new();
    let mut events: Vec<Event> = Vec::new();

    search(
        &ctx,
        &mut memo,
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

fn memo_key(board: Board, depth: usize, hold: Option<Piece>) -> (u64, u64) {
    let hold_code = hold.map_or(0u64, |p| p as u64 + 1);
    (board.0, (depth as u64) << 4 | hold_code)
}

fn is_goal_board(board: Board, two_l: bool) -> bool {
    board == PC_4 || (two_l && board == PC_2)
}

#[allow(clippy::too_many_arguments)]
fn search(
    ctx: &Ctx,
    memo: &mut Memo,
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

    if is_goal_board(board, ctx.two_l)
        && save_ok(ctx.saves, hold, queue.get(depth).copied())
    {
        if !best.found || acc > best.score {
            best.found = true;
            best.score = acc;
            best.moves = path.clone();
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

    let key = memo_key(board, depth, hold);
    if memo.dead.contains(&key) {
        return false;
    }

    if best.found {
        let remaining = (40 - cells) / 4;
        if acc + remaining * MAX_EVENT_SCORE <= best.score {
            return false;
        }
    }

    let active = queue.get(depth).copied();
    let mut reachable = false;

    if let Some(piece) = active {
        reachable |= place(
            ctx,
            memo,
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
                    reachable |= place(
                        ctx,
                        memo,
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
                    reachable |= place(
                        ctx,
                        memo,
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
        reachable |= place(
            ctx,
            memo,
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
fn place(
    ctx: &Ctx,
    memo: &mut Memo,
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
    let can_continue = queue
        .len()
        .saturating_sub(depth_after)
        + in_hold
        >= k + extra;
    let goal_possible = (cells_after == 40
        && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied()))
        || (ctx.two_l
            && cells_after == 20
            && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied()));

    if !can_continue && !goal_possible {
        return false;
    }

    let mut reachable = false;
    for m in &moves(board, piece) {
        let mut next = board;
        next |= m.mask();
        let lines = (next.line_clears() as u32)
            .saturating_sub(board.line_clears() as u32);
        next = next.clearshift();

        if outside_playfield(next) {
            continue;
        }
        if pruned(next) {
            continue;
        }

        if !can_continue {
            let next_goal = is_goal_board(next, ctx.two_l);
            if !(next_goal
                && save_ok(ctx.saves, hold_after, queue.get(depth_after).copied()))
            {
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
            b2b: if lines > 0 {
                difficult
            } else {
                state.b2b
            },
            combo: combo_val,
        };

        path.push(m);
        events.push(event);
        if search(
            ctx,
            memo,
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
        path.pop();
    }

    reachable
}