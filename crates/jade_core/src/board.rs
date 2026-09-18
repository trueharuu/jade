use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::ops::BitXor;
use std::ops::BitXorAssign;
use std::ops::Not;

use crate::header::WIDTH;
use crate::header::dx_mask;

/// A single, row-major, 10x6 bitboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board(u64);

impl Board {
    /// Returns an empty [`Board`].
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a [`Board`] from its raw 64-bit representation.
    #[inline]
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the total number of filled cells in the board.
    #[inline]
    #[must_use]
    pub const fn popcount(self) -> u32 {
        self.0.count_ones()
    }

    /// Shifts every lane by `(dx, dy)`.
    ///
    /// `dy > 0` shifts up; `dy < 0` shifts down. `dx > 0` shifts right;
    /// `dx < 0` shifts left. Bits shifted past an edge are dropped.
    #[inline]
    #[must_use]
    pub const fn shifted(&self, dx: i32, dy: i32) -> Self {
        let dy_shift = dy * 10;

        let v = if dy_shift == 0 {
            self.0
        } else if dy_shift > 0 {
            self.0 << dy_shift
        } else {
            self.0 >> (-dy_shift)
        };

        let out = if dx == 0 {
            v
        } else if dx > 0 {
            v << dx as u64
        } else {
            v >> (-dx) as u64
        } & dx_mask(dx);

        Self(out)
    }

    /// Returns whether the cell at `(x, y)` is set.
    #[inline]
    #[must_use]
    pub const fn get(&self, x: i32, y: i32) -> bool {
        let bit = 1u64 << (y * 10 + x) as u32;
        self.0 & bit != 0
    }

    /// Sets the cell `(x, y)` in lane `lane`.
    #[inline]
    pub const fn set(&mut self, x: i32, y: i32) {
        self.0 |= 1u64 << (y * 10 + x) as u32;
    }

    /// Returns the occupied height of lane `lane`, or `0` if it is empty.
    #[inline]
    #[must_use]
    pub const fn height(&self) -> i32 {
        if self.0 != 0 {
            let idx = 64 - 1 - self.0.leading_zeros() as i32;
            return idx / WIDTH + 1;
        }

        0
    }
}

impl BitAnd for Board {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for Board {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitXor for Board {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitAndAssign for Board {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for Board {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for Board {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for Board {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl From<u64> for Board {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
