use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::score::Event;

pub struct Decision {
    pub cost: u64,
    pub cap: u64,
    pub leaves: u64,
    pub kind: DecisionKind,
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
        events: Vec<Event>,
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
    pub const fn leaves(&self) -> u64 {
        self.leaves
    }

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

    #[inline]
    #[must_use]
    pub const fn solve(cost: u64, cap: u64, leaves: u64, steps: Vec<Move>, events: Vec<Event>) -> Self {
        Self {
            cost,
            cap,
            leaves,
            kind: DecisionKind::Solve { steps, events },
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
