//! Pieces, their filled cells, and their rotation data.

use std::fmt::Display;
use std::marker::ConstParamTy;
use std::str::FromStr;

use crate::rotation::Rotation;

/// A single tetromino type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ConstParamTy)]
#[allow(missing_docs)]
pub enum Piece {
    T = 0,
    I = 1,
    J = 2,
    L = 3,
    O = 4,
    S = 5,
    Z = 6,
}

impl Piece {
    /// Number of tetromino kinds.
    pub const NB: usize = 7;
    /// All tetromino kinds in canonical index order.
    pub const ALL: [Piece; Self::NB] = [
        Piece::T,
        Piece::I,
        Piece::J,
        Piece::L,
        Piece::O,
        Piece::S,
        Piece::Z,
    ];

    /// Converts a compact integer to a piece, returning `None` when out of
    /// range.
    #[inline]
    #[must_use]
    pub const fn from_u8(word: u8) -> Option<Self> {
        match word {
            0 => Some(Piece::T),
            1 => Some(Piece::I),
            2 => Some(Piece::J),
            3 => Some(Piece::L),
            4 => Some(Piece::O),
            5 => Some(Piece::S),
            6 => Some(Piece::Z),
            _ => None,
        }
    }

    /// Three non-origin mino offsets for the spawn orientation.
    #[inline]
    #[must_use]
    pub const fn base_cells(self) -> [(i8, i8); 3] {
        match self {
            Piece::I => [(-1, 0), (1, 0), (2, 0)],
            Piece::O => [(1, 0), (0, 1), (1, 1)],
            Piece::T => [(-1, 0), (1, 0), (0, 1)],
            Piece::L => [(-1, 0), (1, 0), (1, 1)],
            Piece::J => [(-1, 0), (1, 0), (-1, 1)],
            Piece::S => [(-1, 0), (0, 1), (1, 1)],
            Piece::Z => [(-1, 1), (0, 1), (1, 0)],
        }
    }

    /// Maps from an arbitrary [`Rotation`] to its canonical representative, if
    /// this piece has symmetry.
    #[inline]
    #[must_use]
    pub const fn canonical_rotation(self, rot: Rotation) -> Rotation {
        match (self, rot) {
            (Piece::O, _) | (Piece::I | Piece::S | Piece::Z, Rotation::South) => Rotation::North,
            (Piece::I | Piece::S | Piece::Z, Rotation::West) => Rotation::East,
            _ => rot,
        }
    }

    /// Returns `true` for pieces with 90-degree rotational symmetry.
    #[inline]
    #[must_use]
    pub const fn group1(self) -> bool {
        matches!(self, Piece::O)
    }

    /// Returns `true` for pieces with 180-degree rotational symmetry.
    #[inline]
    #[must_use]
    pub const fn group2(self) -> bool {
        matches!(self, Piece::I | Piece::S | Piece::Z)
    }

    /// Returns `true` for pieces with no rotational symmetries.
    #[inline]
    #[must_use]
    pub const fn group4(self) -> bool {
        matches!(self, Piece::T | Piece::J | Piece::L)
    }

    /// Returns the number of canonical rotations for this piece.
    #[inline]
    #[must_use]
    pub const fn groups(self) -> usize {
        match self {
            Piece::O => 1,
            Piece::I | Piece::S | Piece::Z => 2,
            Piece::T | Piece::J | Piece::L => 4,
        }
    }

    /// Returns the total search size for this piece.
    #[inline]
    #[must_use]
    pub const fn search_size(self) -> usize {
        match self {
            Self::O => 1,
            _ => 4,
        }
    }

    /// Returns translation offsets needed to align canonical rotation frames.
    #[inline]
    #[must_use]
    pub const fn canonical_offset(self, r: Rotation) -> (i32, i32) {
        match (self, r) {
            (Piece::I, Rotation::West) => (0, -1),
            (Piece::S | Piece::Z, Rotation::South) => (0, 1),
            (Piece::I, Rotation::South) | (Piece::S | Piece::Z, Rotation::West) => (1, 0),
            _ => (0, 0),
        }
    }

    /// Spawn height adjustment used by spawn placement logic.
    #[inline]
    #[must_use]
    pub const fn h_spawn(self) -> i32 {
        if matches!(self, Piece::I) {
            2
        } else if matches!(self, Piece::O) {
            0
        } else {
            1
        }
    }

    /// Placement height adjustment used by grounded placement logic.
    #[inline]
    #[must_use]
    pub const fn h_place(self) -> i32 {
        2 + (matches!(self, Piece::I) as i32) - (matches!(self, Piece::O) as i32)
    }

    /// Generation height adjustment used by placement generation.
    #[inline]
    #[must_use]
    pub const fn h_gen(self) -> i32 {
        if matches!(self, Piece::I | Piece::T) {
            2
        } else if matches!(self, Piece::O) {
            0
        } else {
            1
        }
    }
}

impl Display for Piece {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Piece::T => write!(f, "T"),
            Piece::I => write!(f, "I"),
            Piece::J => write!(f, "J"),
            Piece::L => write!(f, "L"),
            Piece::O => write!(f, "O"),
            Piece::S => write!(f, "S"),
            Piece::Z => write!(f, "Z"),
        }
    }
}

impl FromStr for Piece {
    type Err = String;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "T" => Ok(Piece::T),
            "I" => Ok(Piece::I),
            "J" => Ok(Piece::J),
            "L" => Ok(Piece::L),
            "O" => Ok(Piece::O),
            "S" => Ok(Piece::S),
            "Z" => Ok(Piece::Z),
            _ => Err(format!("invalid piece: {s}")),
        }
    }
}
