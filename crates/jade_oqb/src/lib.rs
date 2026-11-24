pub mod bag;
pub mod value;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;

use jade_solve::best::best;
use jade_solve::saves::Saves;
use jade_solve::solve::reachable;

use crate::bag::Bag;
use crate::value::Value;

pub struct Decision<V: Value> {
    cost: V,
    cap: V,
    kind: DecisionKind<V>,
}

pub enum DecisionKind<V: Value> {
    Branch {
        field: Board,
        piece: Piece,
        children: [Option<Box<Decision<V>>>; Piece::NB],
    },

    Solve {
        steps: Vec<Move>,
    },

    Capped,
}

impl<V: Value> Decision<V> {
    #[inline]
    pub const fn cost(&self) -> V {
        self.cost
    }

    #[inline]
    pub const fn cap(&self) -> V {
        self.cap
    }

    #[inline]
    pub const fn capped(cost: V, cap: V) -> Self {
        Self {
            cost,
            cap,
            kind: DecisionKind::Capped,
        }
    }

    #[inline]
    pub fn branch(field: Board, piece: Piece, cap: V) -> Self {
        Self {
            cost: V::zero(),
            cap,
            kind: DecisionKind::Branch {
                field,
                piece,
                children: core::array::from_fn(|_| None),
            },
        }
    }

    #[inline]
    pub const fn solve(cost: V, cap: V, steps: Vec<Move>) -> Self {
        Self {
            cost,
            cap,
            kind: DecisionKind::Solve { steps },
        }
    }

    pub fn set_child(&mut self, piece: Piece, child: Decision<V>) {
        let DecisionKind::Branch { children, .. } = &mut self.kind else {
            unreachable!("children are only valid on branch decisions");
        };
        children[piece as usize] = Some(Box::new(child));
    }
}

#[must_use]
pub fn cutoffs<V: Value>(placed: usize, see: usize, bag: Bag) -> Vec<V> {
    let count = (12 - placed as isize - see as isize).max(1) as usize;
    let mut cutoffs = vec![V::zero(); count];
    *cutoffs.last_mut().expect("at least one cutoff") = V::one();

    for i in (0..count - 1).rev() {
        let n = (bag.count() + 7 - i) % 7;
        cutoffs[i] = cutoffs[i + 1].mul_small(if n == 0 { 7 } else { n });
    }

    cutoffs
}

/// Evaluates a fully known suffix.
///
/// Returns `(0, Some(moves))` when a perfect clear exists, and `(1, None)`
/// otherwise.
#[must_use]
pub fn solve_suffix(
    field: Board,
    _hold: Option<Piece>,
    queue: Queue,
    two_l: bool,
    saves: Saves,
    hold_enabled: bool,
) -> (u64, Option<Vec<Move>>) {
    if !reachable(field, queue.as_slice(), two_l, saves, hold_enabled) {
        return (1, None);
    }
    let result = best(field, queue.as_slice(), two_l, saves, hold_enabled).unwrap();
    (0, Some(result.moves))
}

#[must_use]
pub fn oqb(_field: Board, _hold: Option<Piece>, _queue: Queue, _bag: Bag) -> Decision<u64> {
    todo!()
}
