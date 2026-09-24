use jade_core::board::Board;
use jade_core::header::LINES;
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
            x: 0,
            y: 0,
        }
    }
}

// An iterator over the moves in a [`Moves`] buffer.
pub struct MovesIter<'a> {
    moves: &'a Moves,
    rotation: usize,
    x: i32,
    y: i32,
}

impl Iterator for MovesIter<'_> {
    type Item = Move;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.rotation < Rotation::NB {
            // let board = match self.spin {
            //     0 => &self.moves.none[self.rotation],
            //     1 => &self.moves.mini[self.rotation],
            //     2 => &self.moves.full[self.rotation],
            //     _ => unreachable!(),
            // };
            let board = self.moves.mask[self.rotation];

            while self.y < LINES {
                while self.x < WIDTH {
                    if board.get(self.x, self.y) {
                        let mv = Move::new(
                            self.moves.piece,
                            self.x,
                            self.y,
                            Rotation::from_u8(self.rotation as u8),
                        );
                        self.x += 1;
                        return Some(mv);
                    }
                    self.x += 1;
                }
                self.x = 0;
                self.y += 1;
            }
            self.y = 0;
            self.rotation += 1;
        }
        None
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
