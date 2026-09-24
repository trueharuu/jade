//! Rotation states and orientation transforms.
use std::marker::ConstParamTy;

/// Cardinal orientation for piece geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstParamTy)]
pub enum Rotation {
    /// Spawn orientation.
    North = 0,
    /// 90-degree clockwise from spawn.
    East = 1,
    /// 180-degree rotation from spawn.
    South = 2,
    /// 90-degree counterclockwise from spawn.
    West = 3,
}

impl Rotation {
    /// Number of rotation states.
    pub const NB: usize = 4;
    /// All rotation states in index order.
    pub const ALL: [Rotation; Self::NB] = [
        Rotation::North,
        Rotation::East,
        Rotation::South,
        Rotation::West,
    ];

    /// Returns the clockwise successor rotation.
    #[inline]
    #[must_use]
    pub const fn cw(self) -> Self {
        match self {
            Rotation::North => Rotation::East,
            Rotation::East => Rotation::South,
            Rotation::South => Rotation::West,
            Rotation::West => Rotation::North,
        }
    }

    /// Returns the counterclockwise successor rotation.
    #[inline]
    #[must_use]
    pub const fn ccw(self) -> Self {
        match self {
            Rotation::North => Rotation::West,
            Rotation::East => Rotation::North,
            Rotation::South => Rotation::East,
            Rotation::West => Rotation::South,
        }
    }

    /// Returns the half-turn successor rotation.
    #[inline]
    #[must_use]
    pub const fn flip(self) -> Self {
        match self {
            Rotation::North => Rotation::South,
            Rotation::East => Rotation::West,
            Rotation::South => Rotation::North,
            Rotation::West => Rotation::East,
        }
    }

    /// Converts a compact integer to a [`Rotation`], wrapping the input modulo
    /// 4.
    #[inline]
    #[must_use]
    pub const fn from_u8(word: u8) -> Self {
        match word & 3 {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            _ => Self::West,
        }
    }
}

/// Converts a rotation index to a [`Rotation`] at compile time.
#[macro_export]
macro_rules! rot_idx {
    ($i:expr) => {
        match $i & 3 {
            0 => $crate::rotation::Rotation::North,
            1 => $crate::rotation::Rotation::East,
            2 => $crate::rotation::Rotation::South,
            3 => $crate::rotation::Rotation::West,
            #[allow(unsafe_code)]
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    };
}
