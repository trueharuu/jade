use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;
use jade_core::score::Spin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moves {
    pub piece: Piece,
    pub none: [Board; Rotation::NB],
    pub mini: [Board; Rotation::NB],
    pub full: [Board; Rotation::NB],
}

impl Moves {
    /// Creates a new, empty `Moves` result for the given piece.
    #[inline]
    #[must_use]
    pub const fn empty(piece: Piece) -> Self {
        Self {
            none: [Board::empty(); Rotation::NB],
            mini: [Board::empty(); Rotation::NB],
            full: [Board::empty(); Rotation::NB],
            piece,
        }
    }

    /// Inserts a new [`Move`] into the buffer.
    /// Returns `true` if the move was not already present, `false` otherwise.
    #[inline]
    #[must_use]
    pub const fn insert(&mut self, mv: Move) -> bool {
        let r = mv.rotation() as usize;
        let mask = match mv.spin() {
            Spin::None => &mut self.none[r],
            Spin::Mini => &mut self.mini[r],
            Spin::Full => &mut self.full[r],
        };

        if mask.get(mv.x(), mv.y()) {
            false
        } else {
            mask.set(mv.x(), mv.y());
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
            spin: 0,
            rotation: 0,
            x: 0,
            y: 0,
        }
    }
}

// An iterator over the moves in a [`Moves`] buffer.
pub struct MovesIter<'a> {
    moves: &'a Moves,
    spin: usize,
    rotation: usize,
    x: i32,
    y: i32,
}

impl Iterator for MovesIter<'_> {
    type Item = Move;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        const SPINS: [Spin; 3] = [Spin::None, Spin::Mini, Spin::Full];

        while self.spin < 3 {
            while self.rotation < Rotation::NB {
                while self.y < jade_core::header::LINES {
                    while self.x < jade_core::header::WIDTH {
                        let board = match self.spin {
                            0 => self.moves.none[self.rotation],
                            1 => self.moves.mini[self.rotation],
                            _ => self.moves.full[self.rotation],
                        };
                        if board.get(self.x, self.y) {
                            let mv = Move::new(
                                self.moves.piece,
                                self.x,
                                self.y,
                                Rotation::from_u8(self.rotation as u8),
                                SPINS[self.spin],
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
            self.rotation = 0;
            self.spin += 1;
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
