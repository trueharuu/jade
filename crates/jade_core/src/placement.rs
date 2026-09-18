use crate::board::Board;
use crate::piece::Piece;
use crate::rotation::Rotation;

// x=0..9 (4 bits)
// y=0..7 (3 bits)
// rotation=0..3 (2 bits)
// piece=0..6 (3 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u16);

const PIECE_SHIFT: u16 = 0;
const X_SHIFT: u16 = 3;
const Y_SHIFT: u16 = 3 + 4;
const ROT_SHIFT: u16 = 3 + 4 + 3;
const PIECE_MASK: u16 = 0b111;
const X_MASK: u16 = 0b1111;
const Y_MASK: u16 = 0b111;
const ROT_MASK: u16 = 0b11;

impl Move {
    /// Returns an invalid, null [`Move`].
    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Returns the raw 16-bit representation of this [`Move`].
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Creates a [`Move`] from its raw 16-bit representation.
    ///
    /// # Safety
    /// The value must be a valid move.
    #[inline]
    #[must_use]
    pub const unsafe fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Creates a new [`Move`] from its components.
    #[inline]
    #[must_use]
    pub const fn new(piece: Piece, rotation: Rotation, x: i32, y: i32) -> Self {
        let p = piece as u16;
        let r = rotation as u16;
        let x = x as u16;
        let y = y as u16;
        Self(
            (p & PIECE_MASK) << PIECE_SHIFT
                | (x & X_MASK) << X_SHIFT
                | (y & Y_MASK) << Y_SHIFT
                | (r & ROT_MASK) << ROT_SHIFT,
        )
    }

    /// Decodes the [`Piece`] component.
    #[inline]
    #[must_use]
    pub const fn piece(self) -> Piece {
        let p = (self.0 >> PIECE_SHIFT) & PIECE_MASK;
        unsafe { std::mem::transmute(p as u8) }
    }

    /// Decodes the x-position component.
    #[inline]
    #[must_use]
    pub const fn x(self) -> i32 {
        let x = (self.0 >> X_SHIFT) & X_MASK;
        x as i32
    }

    /// Decodes the y-position component.
    #[inline]
    #[must_use]
    pub const fn y(self) -> i32 {
        let y = (self.0 >> Y_SHIFT) & Y_MASK;
        y as i32
    }

    /// Decodes the [`Rotation`] component.
    #[inline]
    #[must_use]
    pub const fn rotation(self) -> Rotation {
        let r = (self.0 >> ROT_SHIFT) & ROT_MASK;
        Rotation::from_u8(r as u8)
    }

    /// Returns the placement mask for this move, or `None` when the move
    /// places the piece out of bounds.
    #[inline]
    #[must_use]
    pub const fn mask(self) -> Board {
        Board::new(crate::data::place_mask(
            self.piece(),
            self.rotation(),
            self.x(),
            self.y(),
        ))
    }

    /// Returns the canonical form of this [`Move`].
    ///
    /// Symmetrical [`Move`]s can have the same [`Move::mask`] result, with
    /// different values:
    /// - [`Piece::T`], [`Piece::J`], and [`Piece::L`] have 4 canonical states
    /// - [`Piece::I`], [`Piece::S`], and [`Piece::Z`] have 2 canonical states
    /// - [`Piece::O`] has 1 canonical state
    #[inline]
    #[must_use]
    pub const fn canonicalize(self) -> Self {
        let piece = self.piece();
        let r = self.rotation();
        let cr = piece.canonical_rotation(r);

        if piece.group4() || (r as u8 == cr as u8) {
            self
        } else {
            let (dx, dy) = piece.canonical_offset(r);
            Self::new(
                piece,
                cr,
                self.x().saturating_sub(dx),
                self.y().saturating_sub(dy),
            )
        }
    }
}
