pub mod bag;
pub mod search;
pub mod decision;

pub use bag::Bag;
pub mod serde;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;
use jade_core::score::Event;
use jade_solve::saves::Saves;

use std::collections::HashMap;
use std::sync::Mutex;

use crate::search::DecisionSearch;
use crate::decision::Decision;
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SuffixKey {
    field: u64,
    queue: Queue,
    hold: Option<Piece>,
    b2b: bool,
    combo: Option<u32>,
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    initial_hold: Option<Piece>,
    initial_b2b: bool,
    initial_combo: Option<u32>,
}

#[allow(clippy::type_complexity)]
static SUFFIX_CACHE: Mutex<Option<HashMap<SuffixKey, Option<(u64, Vec<Event>)>>>> =
    Mutex::new(None);
use crate::decision::DecisionKind;

#[must_use]
pub fn cutoffs(placed: usize, see: usize, bag: Bag) -> Vec<u64> {
    let count = (12 - placed as isize - see as isize).max(1) as usize;
    let mut cutoffs = vec![0u64; count];
    *cutoffs.last_mut().expect("at least one cutoff") = 1;

    for i in (0..count - 1).rev() {
        let n = (bag.count() + 7 - i) % 7;
        cutoffs[i] = cutoffs[i + 1] * if n == 0 { 7 } else { n as u64 };
    }

    cutoffs
}

/// Evaluates a fully known suffix.
///
/// Returns the best score and one move sequence that achieves it.
#[must_use]
pub fn score_suffix(
    field: Board,
    queue: Queue,
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
    initial_hold: Option<Piece>,
    initial_b2b: bool,
    initial_combo: Option<u32>,
    initial_events: Vec<Event>,
    initial_acc: u64,
) -> (u64, Option<Vec<Move>>, Vec<Event>) {
    let key = SuffixKey {
        field: field.0,
        queue,
        hold: initial_hold,
        b2b: initial_b2b,
        combo: initial_combo,
        two_l,
        saves,
        hold_enabled,
        initial_hold,
        initial_b2b,
        initial_combo,
    };

    let cached = {
        let mut guard = SUFFIX_CACHE.lock().unwrap();
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        guard.as_ref().unwrap().get(&key).cloned()
    };

    if let Some(cached) = cached {
        match cached {
            Some((score, _events)) => {
                (initial_acc + score, Some(Vec::new()), initial_events)
            }
            None => (0, None, Vec::new()),
        }
    } else {
        let result = jade_solve::best::best_score_with_state(
            field,
            queue.as_slice(),
            two_l,
            saves,
            hold_enabled,
            initial_hold,
            initial_b2b,
            initial_combo,
        );
        let cached_value = result.as_ref().map(|r| (u64::from(r.score), r.events.clone()));
        {
            let mut guard = SUFFIX_CACHE.lock().unwrap();
            if guard.is_none() {
                *guard = Some(HashMap::new());
            }
            guard.as_mut().unwrap().insert(key, cached_value);
        }
        match result {
            Some(r) => {
                (initial_acc + u64::from(r.score), Some(Vec::new()), initial_events)
            }
            None => (0, None, Vec::new()),
        }
    }
}

pub fn fill_moves(decision: &mut Decision, two_l: bool, saves: Saves, hold_enabled: bool) {
    match &mut decision.kind {
        crate::decision::DecisionKind::Solve {
            field,
            queue,
            hold,
            b2b,
            combo,
            steps,
            events,
        } => {
            if let Some(result) = jade_solve::best::best_with_state(
                *field,
                queue.as_slice(),
                two_l,
                saves,
                hold_enabled,
                *hold,
                *b2b,
                *combo,
            ) {
                *steps = result.moves;
                events.extend(result.events);
            }
        }
        crate::decision::DecisionKind::Branch { children, .. } => {
            for child in children.iter_mut().flatten() {
                fill_moves(child, two_l, saves, hold_enabled);
            }
        }
        crate::decision::DecisionKind::Capped => {}
    }
}

#[must_use]
pub fn oqb(field: Board, queue: Queue, bag: Bag, two_l: bool, threads: usize, initial_hold: Option<Piece>) -> Decision {
    let placed = field.popcount() as usize / 4;
    let see = queue.len();
    let cutoffs = crate::cutoffs(placed, see, bag);
    let search = DecisionSearch::new(two_l, Saves::empty(), true, threads, initial_hold);
    search.solve(field, queue, bag, &cutoffs)
}
