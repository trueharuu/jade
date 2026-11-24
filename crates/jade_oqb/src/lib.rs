pub mod bag;
pub mod search;

pub use bag::Bag;
pub mod serde;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;

use jade_solve::best::best;
use jade_solve::saves::Saves;

use crate::search::DecisionSearch;

pub struct Decision {
    cost: u64,
    cap: u64,
    kind: DecisionKind,
}

pub enum DecisionKind {
    Branch {
        field: Board,
        piece: Piece,
        mv: Move,
        children: [Option<Box<Decision>>; Piece::NB],
    },

    Solve {
        steps: Vec<Move>,
    },

    Capped,
}

impl Decision {
    #[inline]
    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }

    #[inline]
    #[must_use]
    pub const fn cap(&self) -> u64 {
        self.cap
    }

    #[inline]
    #[must_use]
    pub const fn capped(cost: u64, cap: u64) -> Self {
        Self {
            cost,
            cap,
            kind: DecisionKind::Capped,
        }
    }

    #[inline]
    #[must_use]
    pub fn branch(field: Board, piece: Piece, mv: Move, cap: u64) -> Self {
        Self {
            cost: 0,
            cap,
            kind: DecisionKind::Branch {
                field,
                piece,
                mv,
                children: core::array::from_fn(|_| None),
            },
        }
    }

    #[inline]
    #[must_use]
    pub const fn solve(cost: u64, cap: u64, steps: Vec<Move>) -> Self {
        Self {
            cost,
            cap,
            kind: DecisionKind::Solve { steps },
        }
    }

    pub fn set_child(&mut self, piece: Piece, child: Decision) {
        let DecisionKind::Branch { children, .. } = &mut self.kind else {
            unreachable!("children are only valid on branch decisions");
        };
        children[piece as usize] = Some(Box::new(child));
    }

    pub const fn set_cost(&mut self, cost: u64) {
        self.cost = cost;
    }
}

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
) -> (u64, Option<Vec<Move>>) {
    match best(field, queue.as_slice(), two_l, saves, hold_enabled) {
        Some(result) => (u64::from(result.score), Some(result.moves)),
        None => (0, None),
    }
}

#[must_use]
pub fn oqb(field: Board, queue: Queue, bag: Bag) -> Decision {
    let placed = field.popcount() as usize / 4;
    let see = queue.len();
    let cutoffs = crate::cutoffs(placed, see, bag);
    let search = DecisionSearch::new(false, Saves::empty(), true, 1);
    search.solve(field, queue, bag, &cutoffs)
}
