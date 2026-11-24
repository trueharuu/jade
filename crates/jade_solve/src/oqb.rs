use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::queue::Queue;

use crate::Saves;

/// A set of hidden pieces, stored as one bit per piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bag(u8);

impl Bag {
    /// Returns a bag that contains every piece type.
    #[inline]
    #[must_use]
    pub const fn full() -> Self {
        Self((1 << Piece::NB) - 1)
    }

    /// Returns an empty bag.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a bag from a raw mask.
    #[inline]
    #[must_use]
    pub const fn from_mask(mask: u8) -> Self {
        Self(mask & ((1 << Piece::NB) - 1))
    }

    /// Returns the raw mask.
    #[inline]
    #[must_use]
    pub const fn mask(self) -> u8 {
        self.0
    }

    /// Returns the number of piece types in the bag.
    #[inline]
    #[must_use]
    pub const fn count(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns whether the bag contains `piece`.
    #[inline]
    #[must_use]
    pub const fn contains(self, piece: Piece) -> bool {
        self.0 & (1 << (piece as u8)) != 0
    }

    /// Returns a bag without `piece`.
    #[inline]
    #[must_use]
    pub const fn remove(self, piece: Piece) -> Self {
        Self(self.0 & !(1 << (piece as u8)))
    }

    /// Returns whether the bag is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns an iterator over the pieces in the bag.
    #[inline]
    pub fn pieces(self) -> impl Iterator<Item = Piece> {
        (0..Piece::NB as u8).filter_map(move |i| {
            if self.0 & (1 << i) != 0 {
                Piece::from_u8(i)
            } else {
                None
            }
        })
    }
}

pub struct Decision {
    cost: u64,
    cap: u64,
    kind: DecisionKind,
}

pub enum DecisionKind {
    Branch {
        field: Board,
        piece: Piece,
        children: [Option<Box<Decision>>; Piece::NB],
    },

    Solve {
        steps: Vec<Move>,
    },

    Capped,
}

#[must_use]
pub fn cutoffs(placed: usize, see: usize, bag: Bag) -> Vec<u64> {
    let count = (12 - placed as isize - see as isize).max(1) as usize;
    let mut cutoffs = Vec::with_capacity(count);
    *cutoffs.last_mut().unwrap() = 1u64;

    for i in (0..count - 1).rev() {
        let n = (bag.count() + 7 - i) % 7;
        cutoffs[i] = cutoffs[i + 1] * if n == 0 { 7 } else { n as u64 };
    }

    cutoffs
}

pub fn oqb(field: Board, hold: Option<Piece>, queue: Queue, bag: Bag) -> Decision {
    todo!();
}