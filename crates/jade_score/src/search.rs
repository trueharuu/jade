//! Depth-first branch-and-bound search.

use std::cmp::Reverse;

use jade_core::board::Board;
use jade_core::list::List;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::CAP;
use jade_solve::rules::outside_playfield;
use jade_solve::rules::pieces_to_goal;
use jade_solve::rules::pruned;
use jade_solve::saves::save_ok;
use jade_solve::solve::PC_2;
use jade_solve::solve::PC_4;
use jade_solve::solve::moves;

use crate::event::Event;

use super::BestResult;
use super::Options;
use super::score::CELLS_PER_PIECE;
use super::score::PC_CELLS;
use super::score::ScoreState;

/// Cells in the 2-line goal board.
const PC_2_CELLS: u32 = 20;

/// One search position.
#[derive(Clone, Copy)]
pub struct Node {
    board: Board,
    hold: Option<Piece>,
    /// Index of the next queue piece.
    depth: usize,
    score: ScoreState,
    /// Score accumulated on the path to this node.
    acc: u32,
    /// Number of moves placed so far. Indexes the candidate buffer pool.
    ply: usize,
}

/// One way to take a piece: from the queue or from hold.
#[derive(Clone, Copy)]
pub struct Take {
    piece: Piece,
    hold_after: Option<Piece>,
    depth_after: usize,
}

/// A legal move with its result, ready to recurse into.
#[derive(Clone, Copy)]
pub struct Candidate {
    mv: Move,
    board: Board,
    event: Event,
    state: ScoreState,
    score: u32,
}

struct Searcher<'a> {
    opts: Options,
    queue: &'a [Piece],
    best: Option<BestResult>,
    path: List<Move, CAP>,
    events: List<Event, CAP>,
    /// One reusable candidate buffer per ply. Avoids an allocation per node.
    pool: Vec<Vec<Candidate>>,
}

/// Runs the search from a board that is already `clearshift`ed.
pub(super) fn run(board: Board, queue: &[Piece], opts: Options) -> Option<BestResult> {
    let mut searcher = Searcher {
        opts,
        queue,
        best: None,
        path: List::new(),
        events: List::new(),
        pool: Vec::new(),
    };
    searcher.search(Node {
        board,
        // Normalized once. A disabled hold stays `None` on every path.
        hold: if opts.hold_enabled {
            opts.initial_hold
        } else {
            None
        },
        depth: 0,
        score: ScoreState {
            b2b: opts.initial_b2b,
            combo: opts.initial_combo,
        },
        acc: 0,
        ply: 0,
    });
    searcher.best
}

impl Searcher<'_> {
    fn search(&mut self, node: Node) {
        let two_l = self.opts.two_l;
        let saves = self.opts.saves;

        if is_goal_board(node.board, two_l)
            && save_ok(saves, node.hold, self.queue.get(node.depth).copied())
        {
            self.record(node.acc);
            return;
        }

        let cells = node.board.popcount();
        if cells >= PC_CELLS || pruned(node.board) {
            if self.opts.partial && node.ply > 0 {
                self.record(node.acc);
            }
            return;
        }

        let mut placed = false;
        for take in self.takes(&node).into_iter().flatten() {
            if self.place(&node, cells, take) {
                placed = true;
            }
        }
        if !placed && self.opts.partial && node.ply > 0 {
            self.record(node.acc);
        }
    }

    /// Stores a solution when it scores higher than the current best.
    /// Equal scores keep the first solution found.
    fn record(&mut self, score: u32) {
        if self.best.as_ref().is_none_or(|b| score > b.score) {
            self.best = Some(BestResult {
                moves: self.path,
                events: self.events,
                score,
            });
        }
    }

    /// Lists the ways to take the next piece. At most three, no allocation.
    fn takes(&self, node: &Node) -> [Option<Take>; 3] {
        let mut out = [None; 3];
        let Node { hold, depth, .. } = *node;

        let Some(piece) = self.queue.get(depth).copied() else {
            // Queue is empty. Only the hold piece can be placed.
            if let Some(piece) = hold {
                out[0] = Some(Take {
                    piece,
                    hold_after: None,
                    depth_after: depth,
                });
            }
            return out;
        };

        out[0] = Some(Take {
            piece,
            hold_after: hold,
            depth_after: depth + 1,
        });

        if self.opts.hold_enabled {
            match hold {
                Some(h) if h != piece => {
                    out[1] = Some(Take {
                        piece: h,
                        hold_after: Some(piece),
                        depth_after: depth + 1,
                    });
                }
                // Hold is empty: store this piece and place the next one.
                None if depth + 1 < self.queue.len() => {
                    out[1] = Some(Take {
                        piece: self.queue[depth + 1],
                        hold_after: Some(piece),
                        depth_after: depth + 2,
                    });
                }
                _ => {}
            }
        }
        out
    }

    fn place(&mut self, node: &Node, cells: u32, take: Take) -> bool {
        let Take {
            piece,
            hold_after,
            depth_after,
        } = take;
        let two_l = self.opts.two_l;
        let saves = self.opts.saves;
        let cells_after = cells + CELLS_PER_PIECE;

        let save = save_ok(saves, hold_after, self.queue.get(depth_after).copied());
        let k = pieces_to_goal(cells_after, two_l);
        let extra = usize::from(!saves.is_empty() && hold_after.is_none());
        let in_hold = usize::from(hold_after.is_some());
        let can_continue = self.queue.len().saturating_sub(depth_after) + in_hold >= k + extra;
        let goal_possible =
            save && (cells_after == PC_CELLS || (two_l && cells_after == PC_2_CELLS));

        if !can_continue && !goal_possible {
            return false;
        }

        let ply = node.ply;
        if self.pool.len() <= ply {
            self.pool.resize_with(ply + 1, Vec::new);
        }
        let mut cands = std::mem::take(&mut self.pool[ply]);
        cands.clear();

        let moves_list = moves::<true>(node.board, piece);
        for m in &moves_list {
            let Some((next, lines)) = apply(node.board, m) else {
                continue;
            };
            let pc = is_goal_board(next, two_l) && save;
            if !(can_continue || pc) {
                continue;
            }
            let (state, event) = node.score.apply(m.spin(), lines, pc);
            cands.push(Candidate {
                mv: m,
                board: next,
                event,
                state,
                score: event.score(),
            });
        }

        // Try high-scoring moves first. This improves pruning.
        // The sort must stay stable: tie order decides which of several
        // equal-score solutions is kept.
        cands.sort_by_key(|c| Reverse(c.score));

        if cands.is_empty() {
            self.pool[ply] = cands;
            return false;
        }

        for c in cands.drain(..) {
            self.path.push(c.mv);
            self.events.push(c.event);
            self.search(Node {
                board: c.board,
                hold: hold_after,
                depth: depth_after,
                score: c.state,
                acc: node.acc + c.score,
                ply: ply + 1,
            });
            self.events.pop();
            self.path.pop();
        }
        self.pool[ply] = cands;
        true
    }
}

/// Applies a move. Returns the cleared board and the number of lines
/// cleared, or `None` when the result is outside the playfield or pruned.
#[inline]
fn apply(board: Board, m: Move) -> Option<(Board, u32)> {
    let mut next = board;
    next |= m.mask();
    let (n2, cleared) = next.clearshift_with_count();
    let lines = (cleared as u32).saturating_sub(board.line_clears() as u32);
    let next = n2;
    (!outside_playfield(next) && !pruned(next)).then_some((next, lines))
}

#[inline]
fn is_goal_board(board: Board, two_l: bool) -> bool {
    board == PC_4 || (two_l && board == PC_2)
}
