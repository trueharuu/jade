use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;

use jade_solve::moves::moves;
use jade_solve::rules::outside_playfield;
use jade_solve::rules::pruned;
use jade_solve::saves::Saves;

use crate::Decision;
use crate::bag::Bag;
use crate::score_suffix;

#[derive(Clone, Copy)]
struct RootJob {
    field: Board,
    piece: Piece,
    mv: Move,
}

pub struct DecisionSearch {
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    threads: usize,
}

impl DecisionSearch {
    #[must_use]
    pub fn new(two_l: bool, saves: Saves, hold_enabled: bool, threads: usize) -> Self {
        Self {
            two_l,
            saves,
            hold_enabled,
            threads: threads.max(1),
        }
    }

    #[must_use]
    pub fn solve(
        &self,
        field: Board,
        mut queue: Queue,
        bag: Bag,
        cutoffs: &[u64],
    ) -> Decision {
        assert!(
            !cutoffs.is_empty(),
            "decision search needs at least one cutoff"
        );
        assert!(
            cutoffs.len() == 1 || !queue.is_empty(),
            "decision search needs a visible queue piece before a cutoff"
        );

        let first_zero = AtomicUsize::new(usize::MAX);
        tree_imperfect(
            field,
            &mut queue,
            bag,
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

fn tree_imperfect(
    field: Board,
    queue: &mut Queue,
    mut bag: Bag,
    depth: usize,
    cutoffs: &[u64],
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    root_job: Option<usize>,
    first_zero: &AtomicUsize,
    threads: usize,
) -> Option<Decision> {
    if depth >= cutoffs.len().saturating_sub(1) {
        let (cost, steps, events) = score_suffix(field, *queue, two_l, saves, hold_enabled);
        return Some(match steps {
            Some(steps) => Decision::solve(cost, 0, 1, steps, events),
            None => Decision::capped(cost, 0, 1),
        });
    }
    if bag.is_empty() {
        bag = Bag::full();
    }

    let front = queue.as_slice().first().copied()?;
    queue.pop_front();

    let result = if depth == 0 {
        Some(tree_root_parallel(
            field,
            front,
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

    for mv in &moves(field, front) {
        let mut next = field;
        next |= mv.mask();
        next = next.clearshift();
        if outside_playfield(next) || pruned(next) {
            continue;
        }
        let candidate = tree_action(
            next,
            front,
            mv,
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
            if best.cost() == 0 {
                return Some(best);
            }
        }
    }

    Some(best)
}

fn tree_action(
    next_field: Board,
    placed_piece: Piece,
    mv: Move,
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
        queue.push_back(piece);
        let child = tree_imperfect(
            next_field,
            queue,
            bag.remove(piece),
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
        decision.set_child(piece, child);
    }
    Some(decision)
}

fn tree_root_parallel(
    field: Board,
    front: Piece,
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
        jobs.push(RootJob {
            field: next,
            piece: front,
            mv,
        });
    }

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
            scope.spawn(move || {
                loop {
                    let job_index = next_job.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(job_index).copied() else {
                        break;
                    };

                    let Some(candidate) = tree_root_job(
                        job_index,
                        job,
                        queue,
                        bag,
                        cutoffs,
                        first_zero,
                        two_l,
                        saves,
                        hold_enabled,
                    ) else {
                        continue;
                    };

                    // Cancellation is disabled for score maximization.
                    let mut current = best.lock().expect("root decision lock poisoned");
                    if root_candidate_wins(
                        candidate.cost(),
                        job_index,
                        current.decision.cost(),
                        current.job,
                    ) {
                        current.decision = candidate;
                        current.job = job_index;
                    }
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
    bag: Bag,
    cutoffs: &[u64],
    first_zero: &AtomicUsize,
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> Option<Decision> {
    let mut queue = *queue;
    let mut decision = Decision::branch(job.field, job.piece, job.mv, cutoffs[0], cutoffs[0]);

    for piece in bag.pieces() {
        queue.push_back(piece);
        let child = tree_imperfect(
            job.field,
            &mut queue,
            bag.remove(piece),
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

const fn root_candidate_wins(
    candidate_cost: u64,
    candidate_job: usize,
    best_cost: u64,
    best_job: usize,
) -> bool {
    candidate_cost > best_cost || (candidate_cost == best_cost && candidate_job < best_job)
}
