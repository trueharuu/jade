use jade_core::piece::Piece;


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