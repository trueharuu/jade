use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;
use jade_core::score::{Event, Spin};

use jade_solve::moves::moves;
use jade_solve::rules::{outside_playfield, pieces_to_goal, pruned};
use jade_solve::saves::Saves;

use crate::bag::Bag;
use crate::score_suffix;
use crate::Decision;

#[derive(Clone)]
struct ScoreState {
    b2b: bool,
    combo: Option<u32>,
    events: Vec<Event>,
    acc: u64,
}

#[derive(Clone)]
struct RootJob {
    field: Board,
    piece: Piece,
    mv: Move,
    state: ScoreState,
}

pub struct DecisionSearch {
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    threads: usize,
}

impl DecisionSearch {
    pub fn new(two_l: bool, saves: Saves, hold_enabled: bool, threads: usize) -> Self {
        Self {
            two_l,
            saves,
            hold_enabled,
            threads: threads.max(1),
        }
    }

    pub fn solve(
        &self,
        field: Board,
        mut queue: Queue,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Decision {
        assert!(!cutoffs.is_empty(), "decision search needs at least one cutoff");
        assert!(
            cutoffs.len() == 1 || !queue.is_empty(),
            "decision search needs a visible queue piece before a cutoff"
        );

        let first_zero = AtomicUsize::new(usize::MAX);
        tree_imperfect(
            field,
            &mut queue,
            bag,
            None,
            ScoreState {
                b2b: false,
                combo: None,
                events: Vec::new(),
                acc: 0,
            },
            0,
            cutoffs,
            self.two_l,
            self.saves,
            self.hold_enabled,
            None,
            &first_zero,
            self.threads,
        )
        .expect("top-level decision job cannot be cancelled")
    }
}

fn cancelled(_root_job: Option<usize>, _first_zero: &AtomicUsize) -> bool {
    false
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
    let mut events = state.events.clone();
    events.push(event);
    ScoreState {
        b2b: if lines > 0 { difficult } else { state.b2b },
        combo: combo_val,
        events,
        acc: state.acc + event.score() as u64,
    }
}

fn tree_imperfect(
    field: Board,
    queue: &mut Queue,
    mut bag: Bag,
    hold: Option<Piece>,
    state: ScoreState,
    depth: usize,
    cutoffs: &[u64],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    root_job: Option<usize>,
    first_zero: &AtomicUsize,
    threads: usize,
) -> Option<Decision> {
    if cancelled(root_job, first_zero) {
        return None;
    }
    let enough = {
        let cells = field.popcount();
        let k = pieces_to_goal(cells, two_l);
        let need = k + usize::from(hold_enabled);
        queue.len() >= need
    };
    if depth >= cutoffs.len().saturating_sub(1) || enough {
        let (cost, steps, events) = score_suffix(
            field,
            *queue,
            two_l,
            saves,
            hold_enabled,
            hold,
            state.b2b,
            state.combo,
            state.events.clone(),
            state.acc,
        );
        return Some(match steps {
            Some(steps) => Decision::solve(cost, 0, 1, steps, events),
            None => Decision::capped(cost, 0, 1),
        });
    }
    if bag.is_empty() {
        bag = Bag::full();
    }

    let front = match queue.as_slice().first().copied() {
        Some(piece) => piece,
        None => return None,
    };
    queue.pop_front();

    let result = if depth == 0 {
        Some(tree_root_parallel(
            field,
            front,
            hold,
            state,
            queue,
            bag,
            cutoffs,
            two_l,
            saves,
            hold_enabled,
            first_zero,
            threads,
        ))
    } else {
        tree_after_front(
            field,
            front,
            hold,
            state,
            queue,
            depth,
            bag,
            cutoffs,
            two_l,
            saves,
            hold_enabled,
            root_job,
            first_zero,
        )
    };

    queue.push_front(front);
    result
}

fn tree_after_front(
    field: Board,
    front: Piece,
    hold: Option<Piece>,
    state: ScoreState,
    queue: &mut Queue,
    depth: usize,
    bag: Bag,
    cutoffs: &[u64],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    root_job: Option<usize>,
    first_zero: &AtomicUsize,
) -> Option<Decision> {
    let mut best = Decision::capped(0, cutoffs[depth], cutoffs[depth]);

    // Place the active piece.
    for mv in &moves(field, front) {
        let mut next = field;
        next |= mv.mask();
        next = next.clearshift();
        if outside_playfield(next) || pruned(next) {
            continue;
        }
        let new_state = update_state(&state, field, mv);
        let candidate = tree_action(
            next,
            front,
            mv,
            hold,
            new_state,
            queue,
            depth,
            bag,
            cutoffs,
            two_l,
            saves,
            hold_enabled,
            root_job,
            first_zero,
        )?;
        if candidate.cost() > best.cost() {
            best = candidate;
        }
    }

    // Place the held piece.
    if hold_enabled {
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
                    let candidate = tree_action(
                        next,
                        h,
                        mv,
                        Some(front),
                        new_state,
                        queue,
                        depth,
                        bag,
                        cutoffs,
                        two_l,
                        saves,
                        hold_enabled,
                        root_job,
                        first_zero,
                    )?;
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
                let candidate = tree_action(
                    next,
                    next_piece,
                    mv,
                    Some(front),
                    new_state,
                    &mut q,
                    depth,
                    bag,
                    cutoffs,
                    two_l,
                    saves,
                    hold_enabled,
                    root_job,
                    first_zero,
                )?;
                if candidate.cost() > best.cost() {
                    best = candidate;
                }
            }
        }
    }

    Some(best)
}

fn tree_action(
    next_field: Board,
    placed_piece: Piece,
    mv: Move,
    hold: Option<Piece>,
    state: ScoreState,
    queue: &mut Queue,
    depth: usize,
    bag: Bag,
    cutoffs: &[u64],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    root_job: Option<usize>,
    first_zero: &AtomicUsize,
) -> Option<Decision> {
    let mut decision = Decision::branch(next_field, placed_piece, mv, cutoffs[depth], cutoffs[depth]);
    for piece in bag.pieces() {
        if cancelled(root_job, first_zero) {
            return None;
        }
        queue.push_back(piece);
        let child = tree_imperfect(
            next_field,
            queue,
            bag.remove(piece),
            hold,
            state.clone(),
            depth + 1,
            cutoffs,
            two_l,
            saves,
            hold_enabled,
            root_job,
            first_zero,
            1,
        );
        queue.pop_back();
        let child = child?;
        decision.set_cost(decision.cost() + child.cost());
        // store event-adjusted cost by mutating child? Decision cost immutable; create new?
        decision.set_child(piece, child);
    }
    Some(decision)
}

fn tree_root_parallel(
    field: Board,
    front: Piece,
    hold: Option<Piece>,
    state: ScoreState,
    queue: &Queue,
    bag: Bag,
    cutoffs: &[u64],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    first_zero: &AtomicUsize,
    threads: usize,
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
            mv,
            state: new_state,
        });
    }

    // Hold-piece and hold-swap placements are handled by the first job? For simplicity, root only active.
    if jobs.is_empty() {
        return Decision::capped(0, cutoffs[0], cutoffs[0]);
    }

    let next_job = AtomicUsize::new(0);
    let best = Mutex::new(RootBest {
        decision: Decision::capped(0, cutoffs[0], cutoffs[0]),
        job: usize::MAX,
    });
    let workers = threads.min(jobs.len());

    thread::scope(|scope| {
        for _ in 0..workers {
            let best = &best;
            let next_job = &next_job;
            let jobs = &jobs;
            let state = state.clone();
            scope.spawn(move || loop {
                let job_index = next_job.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(job_index).cloned() else {
                    break;
                };
                if cancelled(Some(job_index), first_zero) {
                    break;
                }

                let state = state.clone();
                let Some(candidate) = tree_root_job(
                    job_index,
                    job,
                    queue,
                    hold,
                    state,
                    bag,
                    cutoffs,
                    best,
                    first_zero,
                    two_l,
                    saves,
                    hold_enabled,
                ) else {
                    continue;
                };

                let mut current = best.lock().expect("root decision lock poisoned");
                if root_candidate_wins(candidate.cost(), job_index, current.decision.cost(), current.job) {
                    current.decision = candidate;
                    current.job = job_index;
                }
            });
        }
    });

    best.into_inner()
        .expect("root decision lock poisoned")
        .decision
}

fn tree_root_job(
    job_index: usize,
    job: RootJob,
    queue: &Queue,
    hold: Option<Piece>,
    state: ScoreState,
    bag: Bag,
    cutoffs: &[u64],
    best: &Mutex<RootBest>,
    first_zero: &AtomicUsize,
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> Option<Decision> {
    let mut queue = *queue;
    let mut decision = Decision::branch(job.field, job.piece, job.mv, cutoffs[0], cutoffs[0]);

    for piece in bag.pieces() {
        if cancelled(Some(job_index), first_zero) {
            return None;
        }
        queue.push_back(piece);
        let child = tree_imperfect(
            job.field,
            &mut queue,
            bag.remove(piece),
            hold,
            job.state.clone(),
            1,
            cutoffs,
            two_l,
            saves,
            hold_enabled,
            Some(job_index),
            first_zero,
            1,
        );
        queue.pop_back();
        let child = child?;
        decision.set_cost(decision.cost() + child.cost());
        decision.set_child(piece, child);
    }
    Some(decision)
}

struct RootBest {
    decision: Decision,
    job: usize,
}

fn root_candidate_wins(candidate_cost: u64, candidate_job: usize, best_cost: u64, best_job: usize) -> bool {
    candidate_cost > best_cost || (candidate_cost == best_cost && candidate_job < best_job)
}