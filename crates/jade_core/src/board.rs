use std::cmp::Ordering;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not,
};
use std::simd::num::SimdUint;
use std::simd::Simd;

use crate::header::{dx_mask, MASK, WIDTH};

/// A set of `N` independent 6-row working fields, one per lane.
///
/// Each lane is a 60-bit field, bit index `y * 10 + x`, row 0 at the
/// bottom. In move generation a lane is one working field. Lanes never
/// interact.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct Plane<const N: usize>(pub Simd<u64, N>);

impl<const N: usize> Plane<N> {
    /// An empty plane in every lane.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(Simd::splat(0))
    }

    /// A plane with the same 60-bit value in every lane.
    #[inline]
    #[must_use]
    pub fn splat(value: u64) -> Self {
        Self(Simd::splat(value))
    }

    /// Loads one distinct plane per lane.
    #[inline]
    #[must_use]
    pub fn from_array(planes: [u64; N]) -> Self {
        Self(Simd::from_array(planes))
    }

    /// Extracts every lane.
    #[inline]
    #[must_use]
    pub fn to_array(self) -> [u64; N] {
        self.0.to_array()
    }

    /// Returns `true` when any lane has any bit set.
    #[inline]
    #[must_use]
    pub fn any(self) -> bool {
        self.0.reduce_or() != 0
    }

    /// Counts set bits across all lanes.
    #[inline]
    #[must_use]
    pub fn popcount(self) -> u64 {
        self.0.count_ones().reduce_sum()
    }

    /// Shifts every lane by `(dx, dy)`.
    ///
    /// `dy > 0` shifts up; `dy < 0` shifts down. `dx > 0` shifts right;
    /// `dx < 0` shifts left. Bits shifted past an edge are dropped.
    #[inline]
    #[must_use]
    pub fn shifted(&self, dx: i32, dy: i32) -> Self {
        let dy_shift = dy * 10;

        let v = match dy_shift.cmp(&0) {
            Ordering::Equal => self.0,
            Ordering::Greater => self.0 << Simd::splat(dy_shift as u64),
            Ordering::Less => self.0 >> Simd::splat((-dy_shift) as u64),
        };

        let out = match dx.cmp(&0) {
            Ordering::Equal => v,
            Ordering::Greater => v << Simd::splat(dx as u64),
            Ordering::Less => v >> Simd::splat((-dx) as u64),
        } & Simd::splat(dx_mask(dx));

        Self(out)
    }

    /// Reads the bit at cell `(x, y)` of lane 0.
    #[inline]
    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> bool {
        (self.0[0] >> (y * WIDTH + x) as u32) & 1 == 1
    }

    /// Sets the bit at cell `(x, y)` in every lane.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32) {
        self.0 |= Simd::splat(1u64 << (y * WIDTH + x) as u32);
    }
}

impl<const N: usize> BitAnd for Plane<N> {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl<const N: usize> BitOr for Plane<N> {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl<const N: usize> BitXor for Plane<N> {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl<const N: usize> BitAndAssign for Plane<N> {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl<const N: usize> BitOrAssign for Plane<N> {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl<const N: usize> BitXorAssign for Plane<N> {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl<const N: usize> Not for Plane<N> {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(Simd::splat(MASK) & !self.0)
    }
}
