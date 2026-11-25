use jade_core::board::Board;
use jade_core::list::List;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;
use jade_core::score::Event;
use jade_core::score::Spin;

use jade_solve::moves::moves;
use jade_solve::rules::outside_playfield;
use jade_solve::rules::pieces_to_goal;
use jade_solve::rules::pruned;
use jade_solve::saves::Saves;

use crate::Decision;
use crate::bag::Bag;
use crate::score_suffix;

#[derive(Clone, Copy)]
struct ScoreState {
    b2b: bool,
    combo: Option<u32>,
    acc: u64,
}

#[derive(Clone, Copy)]
struct RootJob {
    field: Board,
    piece: Piece,
    mv: Move,
    state: ScoreState,
    hold: Option<Piece>,
}

#[derive(Clone)]
struct SearchContext {
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
}

pub struct RootBest {
    decision: Decision,
    job: usize,
}

pub struct DecisionSearch {
    context: SearchContext,
    initial_hold: Option<Piece>,
}

impl DecisionSearch {
    #[must_use]
    pub const fn new(
        two_l: bool,
        saves: Saves,
        hold_enabled: bool,
        initial_hold: Option<Piece>,
    ) -> Self {
        Self {
            context: SearchContext {
                two_l,
                saves,
                hold_enabled,
            },
            initial_hold,
        }
    }

    #[must_use]
    pub fn solve(&self, field: Board, mut queue: Queue, bag: Bag, cutoffs: &[u64]) -> Decision {
        assert!(
            !cutoffs.is_empty(),
            "decision search needs at least one cutoff"
        );
        assert!(
            cutoffs.len() == 1 || !queue.is_empty(),
            "decision search needs a visible queue piece before a cutoff"
        );

        self.context
            .tree_imperfect(
                field,
                &mut queue,
                bag,
                self.initial_hold,
                ScoreState {
                    b2b: false,
                    combo: None,
                    acc: 0,
                },
                0,
                cutoffs,
            )
            .expect("top-level decision job cannot be cancelled")
    }
}

impl SearchContext {
    fn tree_imperfect(
        &self,
        field: Board,
        queue: &mut Queue,
        mut bag: Bag,
        hold: Option<Piece>,
        state: ScoreState,
        depth: usize,
        cutoffs: &[u64],
    ) -> Option<Decision> {
        let enough = {
            let cells = field.popcount();
            let k = pieces_to_goal(cells, self.two_l);
            let need = k + usize::from(self.hold_enabled);
            queue.len() >= need
        };
        if depth >= cutoffs.len().saturating_sub(1) || enough {
            let (cost, steps, events) = score_suffix(
                field,
                *queue,
                self.two_l,
                self.saves,
                self.hold_enabled,
                hold,
                state.b2b,
                state.combo,
                List::new(),
                state.acc,
            );
            return Some(match steps {
                Some(steps) => Decision::solve(
                    cost,
                    0,
                    1,
                    field,
                    *queue,
                    hold,
                    state.b2b,
                    state.combo,
                    steps,
                    events,
                ),
                None => Decision::capped(cost, 0, 1),
            });
        }
        if bag.is_empty() {
            bag = Bag::full();
        }

        let front = queue.as_slice().first().copied()?;
        queue.pop_front();

        let result = if depth == 0 {
            Some(self.tree_root_parallel(field, front, hold, state, queue, bag, cutoffs))
        } else {
            Some(self.tree_after_front(field, front, hold, state, queue, depth, bag, cutoffs))
        };

        queue.push_front(front);
        result
    }

    fn tree_after_front(
        &self,
        field: Board,
        front: Piece,
        hold: Option<Piece>,
        state: ScoreState,
        queue: &mut Queue,
        depth: usize,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Decision {
        let mut best = Decision::capped(0, cutoffs[depth], cutoffs[depth]);
        let child_leaves = if depth + 1 < cutoffs.len() {
            cutoffs[depth + 1]
        } else {
            1
        };

        for mv in &moves(field, front) {
            let mut next = field;
            next |= mv.mask();
            next = next.clearshift();
            if outside_playfield(next) || pruned(next) {
                continue;
            }
            let bound =
                bag.count() as u64 * child_leaves * max_leaf_score(next.popcount(), self.two_l);
            if bound <= best.cost() {
                continue;
            }
            let new_state = update_state(&state, field, mv);
            let candidate =
                self.tree_action(next, front, mv, hold, new_state, queue, depth, bag, cutoffs);
            if candidate.cost() > best.cost() {
                best = candidate;
            }
        }

        if self.hold_enabled {
            if let Some(h) = hold {
                if h != front {
                    for mv in &moves(field, h) {
                        let mut next = field;
                        next |= mv.mask();
                        next = next.clearshift();
                        if outside_playfield(next) || pruned(next) {
                            continue;
                        }
                        let bound = bag.count() as u64
                            * child_leaves
                            * max_leaf_score(next.popcount(), self.two_l);
                        if bound <= best.cost() {
                            continue;
                        }
                        let new_state = update_state(&state, field, mv);
                        let candidate = self.tree_action(
                            next,
                            h,
                            mv,
                            Some(front),
                            new_state,
                            queue,
                            depth,
                            bag,
                            cutoffs,
                        );
                        if candidate.cost() > best.cost() {
                            best = candidate;
                        }
                    }
                }
            } else if !queue.as_slice().is_empty() {
                let next_piece = queue.as_slice()[0];
                let mut temp_queue = *queue;
                temp_queue.pop_front();
                for mv in &moves(field, next_piece) {
                    let mut next = field;
                    next |= mv.mask();
                    next = next.clearshift();
                    if outside_playfield(next) || pruned(next) {
                        continue;
                    }
                    let new_state = update_state(&state, field, mv);
                    let mut q = temp_queue;
                    let candidate = self.tree_action(
                        next,
                        next_piece,
                        mv,
                        Some(front),
                        new_state,
                        &mut q,
                        depth,
                        bag,
                        cutoffs,
                    );
                    if candidate.cost() > best.cost() {
                        best = candidate;
                    }
                }
            }
        }

        best
    }

    fn tree_action(
        &self,
        next_field: Board,
        placed_piece: Piece,
        mv: Move,
        hold: Option<Piece>,
        state: ScoreState,
        queue: &mut Queue,
        depth: usize,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Decision {
        let mut decision =
            Decision::branch(next_field, placed_piece, mv, cutoffs[depth], cutoffs[depth]);
        let mut pieces = Queue::new();
        for p in bag.pieces() {
            pieces.push(p);
        }
        let mut results = Vec::new();
        for &piece in pieces.as_slice() {
            let mut q = *queue;
            q.push_back(piece);
            let child = self.tree_imperfect(
                next_field,
                &mut q,
                bag.remove(piece),
                hold,
                state,
                depth + 1,
                cutoffs,
            );
            results.push((piece, child));
        }

        for (piece, child) in results {
            let Some(child) = child else {
                continue;
            };
            decision.set_cost(decision.cost() + child.cost());
            decision.set_child(piece, child);
        }

        decision
    }

    fn tree_root_parallel(
        &self,
        field: Board,
        front: Piece,
        hold: Option<Piece>,
        state: ScoreState,
        queue: &Queue,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Decision {
        let mut jobs: Vec<RootJob> = Vec::new();

        for mv in &moves(field, front) {
            let mut next = field;
            next |= mv.mask();
            next = next.clearshift();
            if outside_playfield(next) || pruned(next) {
                continue;
            }
            let new_state = update_state(&state, field, mv);
            jobs.push(RootJob {
                field: next,
                piece: front,
                mv: mv,
                state: new_state,
                hold,
            });
        }

        if self.hold_enabled {
            if let Some(h) = hold {
                if h != front {
                    for mv in &moves(field, h) {
                        let mut next = field;
                        next |= mv.mask();
                        next = next.clearshift();
                        if outside_playfield(next) || pruned(next) {
                            continue;
                        }
                        let new_state = update_state(&state, field, mv);
                        jobs.push(RootJob {
                            field: next,
                            piece: h,
                            mv: mv,
                            state: new_state,
                            hold: Some(front),
                        });
                    }
                }
            } else if !queue.as_slice().is_empty() {
                let next_piece = queue.as_slice()[0];
                for mv in &moves(field, next_piece) {
                    let mut next = field;
                    next |= mv.mask();
                    next = next.clearshift();
                    if outside_playfield(next) || pruned(next) {
                        continue;
                    }
                    let new_state = update_state(&state, field, mv);
                    jobs.push(RootJob {
                        field: next,
                        piece: next_piece,
                        mv: mv,
                        state: new_state,
                        hold: Some(front),
                    });
                }
            }
        }

        if jobs.is_empty() {
            return Decision::capped(0, cutoffs[0], cutoffs[0]);
        }

        let mut best = RootBest {
            decision: Decision::capped(0, cutoffs[0], cutoffs[0]),
            job: usize::MAX,
        };

        for (job_index, job) in jobs.iter().copied().enumerate() {
            let Some(candidate) = self.tree_root_job(job, queue, bag, cutoffs) else {
                continue;
            };

            if root_candidate_wins(candidate.cost(), job_index, best.decision.cost(), best.job) {
                best.decision = candidate;
                best.job = job_index;
            }
        }

        best.decision
    }

    fn tree_root_job(
        &self,
        job: RootJob,
        queue: &Queue,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Option<Decision> {
        let queue = *queue;
        let mut decision = Decision::branch(job.field, job.piece, job.mv, cutoffs[0], cutoffs[0]);

        let mut pieces = Queue::new();
        for p in bag.pieces() {
            pieces.push(p);
        }
        let mut results = Vec::new();
        for &piece in pieces.as_slice() {
            let mut q = queue;
            q.push_back(piece);
            let child = self.tree_imperfect(
                job.field,
                &mut q,
                bag.remove(piece),
                job.hold,
                job.state,
                1,
                cutoffs,
            );
            results.push((piece, child));
        }

        for (piece, child) in results {
            let Some(child) = child else {
                continue;
            };
            decision.set_cost(decision.cost() + child.cost());
            decision.set_child(piece, child);
        }
        Some(decision)
    }
}

fn update_state(state: &ScoreState, board: Board, mv: Move) -> ScoreState {
    let lines = (board | mv.mask()).line_clears() - board.line_clears();
    let lines = lines.max(0) as u32;
    let difficult = mv.spin() != Spin::None || lines == 4;
    let combo_val = if lines > 0 {
        Some(state.combo.map_or(0, |c| c + 1))
    } else {
        None
    };
    let event = Event {
        b2b: if lines > 0 {
            state.b2b && difficult
        } else {
            false
        },
        combo: combo_val,
        spin: mv.spin(),
        lines,
        pc: false,
    };
    ScoreState {
        b2b: if lines > 0 { difficult } else { state.b2b },
        combo: combo_val,
        acc: state.acc + u64::from(event.score()),
    }
}

const fn max_leaf_score(cells: u32, two_l: bool) -> u64 {
    let r = pieces_to_goal(cells, two_l) as u64;
    if r == 0 { 5400 } else { 2400 * (r - 1) + 5400 }
}

const fn root_candidate_wins(
    candidate_cost: u64,
    candidate_job: usize,
    best_cost: u64,
    best_job: usize,
) -> bool {
    candidate_cost > best_cost || (candidate_cost == best_cost && candidate_job < best_job)
}
