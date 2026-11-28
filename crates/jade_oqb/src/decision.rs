//! Decisions.

use jade_core::board::Board;
use jade_core::list::List;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::CAP;
use jade_core::queue::Queue;
use jade_score::event::Event;

/// Decision.
pub struct Decision {
    /// Cost.
    pub cost: u64,
    /// Cap.
    pub cap: u64,
    /// Leaves.
    pub leaves: u64,
    /// Kind.
    pub kind: DecisionKind,
}

/// Decision kind.
pub enum DecisionKind {
    /// Branch.
    Branch {
        /// Field.
        field: Board,
        /// Piece.
        piece: Piece,
        /// Move.
        mv: Move,
        /// Children.
        children: [Option<Box<Decision>>; Piece::NB],
    },

    /// Solve.
    Solve {
        /// Field.
        field: Board,
        /// Queue.
        queue: Queue,
        /// Hold.
        hold: Option<Piece>,
        /// B2B.
        b2b: bool,
        /// Combo.
        combo: Option<u32>,
        /// Steps.
        steps: List<Move, CAP>,
        /// Events.
        events: List<Event, CAP>,
    },

    /// Partial.
    Partial {
        /// Field.
        field: Board,
        /// Queue.
        queue: Queue,
        /// Hold.
        hold: Option<Piece>,
        /// B2B.
        b2b: bool,
        /// Combo.
        combo: Option<u32>,
        /// Steps.
        steps: List<Move, CAP>,
        /// Events.
        events: List<Event, CAP>,
    },

    /// Capped.
    Capped,
}

impl Decision {
    /// Cost.
    #[inline]
    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }

    /// Cap.
    #[inline]
    #[must_use]
    pub const fn cap(&self) -> u64 {
        self.cap
    }

    /// Leaves.
    #[inline]
    #[must_use]
    pub const fn leaves(&self) -> u64 {
        self.leaves
    }

    /// Capped.
    #[inline]
    #[must_use]
    pub const fn capped(cost: u64, cap: u64, leaves: u64) -> Self {
        Self {
            cost,
            cap,
            leaves,
            kind: DecisionKind::Capped,
        }
    }

    /// Branch.
    #[inline]
    #[must_use]
    pub fn branch(field: Board, piece: Piece, mv: Move, cap: u64, leaves: u64) -> Self {
        Self {
            cost: 0,
            cap,
            leaves,
            kind: DecisionKind::Branch {
                field,
                piece,
                mv,
                children: core::array::from_fn(|_| None),
            },
        }
    }

    /// Solve.
    #[inline]
    #[must_use]
    pub const fn solve(
        cost: u64,
        cap: u64,
        leaves: u64,
        field: Board,
        queue: Queue,
        hold: Option<Piece>,
        b2b: bool,
        combo: Option<u32>,
        steps: List<Move, CAP>,
        events: List<Event, CAP>,
    ) -> Self {
        Self {
            cost,
            cap,
            leaves,
            kind: DecisionKind::Solve {
                field,
                queue,
                hold,
                b2b,
                combo,
                steps,
                events,
            },
        }
    }

    /// Partial.
    #[inline]
    #[must_use]
    pub const fn partial(
        cost: u64,
        cap: u64,
        leaves: u64,
        field: Board,
        queue: Queue,
        hold: Option<Piece>,
        b2b: bool,
        combo: Option<u32>,
        steps: List<Move, CAP>,
        events: List<Event, CAP>,
    ) -> Self {
        Self {
            cost,
            cap,
            leaves,
            kind: DecisionKind::Partial {
                field,
                queue,
                hold,
                b2b,
                combo,
                steps,
                events,
            },
        }
    }

    /// Set child.
    pub fn set_child(&mut self, piece: Piece, child: Decision) {
        let DecisionKind::Branch { children, .. } = &mut self.kind else {
            unreachable!("children are only valid on branch decisions");
        };
        children[piece as usize] = Some(Box::new(child));
    }

    /// Set cost.
    pub const fn set_cost(&mut self, cost: u64) {
        self.cost = cost;
    }
}
