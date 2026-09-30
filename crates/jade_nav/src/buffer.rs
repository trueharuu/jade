use jade_core::board::Board;
use jade_core::header::WIDTH;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moves {
    pub piece: Piece,
    pub mask: [Board; Rotation::NB],
}

impl Moves {
    /// Creates a new, empty `Moves` result for the given piece.
    #[inline]
    #[must_use]
    pub const fn empty(piece: Piece) -> Self {
        Self {
            mask: [Board::empty(); Rotation::NB],
            piece,
        }
    }

    /// Inserts a new [`Move`] into the buffer.
    /// Returns `true` if the move was not already present, `false` otherwise.
    #[inline]
    #[must_use]
    pub const fn insert(&mut self, mv: Move) -> bool {
        let r = mv.rotation() as usize;

        if self.mask[r].get(mv.x(), mv.y()) {
            false
        } else {
            self.mask[r].set(mv.x(), mv.y());
            true
        }
    }

    /// Returns an iterator over all moves stored in this buffer, in ascending
    /// `(rotation, y, x)` order.
    #[inline]
    #[must_use]
    pub const fn iter(&self) -> MovesIter<'_> {
        MovesIter {
            moves: self,
            rotation: 0,
            mask: 0,
        }
    }
}

// An iterator over the moves in a [`Moves`] buffer.
pub struct MovesIter<'a> {
    moves: &'a Moves,
    rotation: usize,
    mask: u64,
}

impl Iterator for MovesIter<'_> {
    type Item = Move;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.mask == 0 {
                if self.rotation >= Rotation::NB {
                    return None;
                }
                self.mask = self.moves.mask[self.rotation].0;
                self.rotation += 1;
                continue;
            }
            let tz = self.mask.trailing_zeros() as i32;
            let x = tz % WIDTH;
            let y = tz / WIDTH;
            self.mask &= self.mask - 1;
            return Some(Move::new(
                self.moves.piece,
                x,
                y,
                Rotation::from_u8((self.rotation - 1) as u8),
            ));
        }
    }
}

impl<'a> IntoIterator for &'a Moves {
    type Item = Move;
    type IntoIter = MovesIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
