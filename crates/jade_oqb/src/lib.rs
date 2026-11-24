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

use jade_solve::best::best;
use jade_solve::saves::Saves;

use crate::search::DecisionSearch;
use crate::decision::Decision;
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
    match jade_solve::best::best_with_state(
        field,
        queue.as_slice(),
        two_l,
        saves,
        hold_enabled,
        initial_hold,
        initial_b2b,
        initial_combo,
    ) {
        Some(result) => {
            let mut events = initial_events;
            events.extend(result.events);
            (initial_acc + u64::from(result.score), Some(result.moves), events)
        }
        None => (0, None, Vec::new()),
    }
}

#[must_use]
pub fn oqb(field: Board, queue: Queue, bag: Bag, threads: usize) -> Decision {
    let placed = field.popcount() as usize / 4;
    let see = queue.len();
    let cutoffs = crate::cutoffs(placed, see, bag);
    let search = DecisionSearch::new(false, Saves::empty(), true, threads);
    search.solve(field, queue, bag, &cutoffs)
}
