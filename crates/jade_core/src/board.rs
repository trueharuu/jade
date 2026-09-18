use std::cmp::Ordering;
use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::ops::BitXor;
use std::ops::BitXorAssign;
use std::ops::Not;
use std::simd::Simd;
use std::simd::num::SimdUint;

use crate::header::MASK;
use crate::header::WIDTH;
use crate::header::dx_mask;

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

    /// Returns whether the cell at `(x, y)` in lane `lane` is set.
    #[inline]
    #[must_use]
    pub fn get(&self, lane: usize, x: i32, y: i32) -> bool {
        debug_assert!(lane < N, "lane {lane} out of bounds for Plane<{N}>");
        debug_assert!((0..WIDTH).contains(&x), "x {x} out of bounds for Plane<{N}>");
        debug_assert!((0..6).contains(&y), "y {y} out of bounds for Plane<{N}>");
        let bit = 1u64 << (y * 10 + x) as u32;
        self.0[lane] & bit != 0
    }

    /// Returns a bit-vector of all lanes that have the cell at `(x, y)` set.
    #[inline]
    #[must_use]
    pub fn get_many(&self, x: i32, y: i32) -> Simd<u64, N> {
        let bit = 1u64 << (y * 10 + x) as u32;
        self.0 & Simd::splat(bit)
    }

    /// Sets the cell `(x, y)` in lane `lane`.
    #[inline]
    pub fn set(&mut self, lane: usize, x: i32, y: i32) {
        let bit = 1u64 << (y * 10 + x) as u32;
        self.0[lane] |= bit;
    }

    /// Sets the cell `(x, y)` for all lanes in the bit-vector `mask`.
    #[inline]
    pub fn set_many(&mut self, mask: u64, x: i32, y: i32) {
        for i in 0..N {
            if (mask >> i) & 1 != 0 {
                let bit = 1u64 << (y * 10 + x) as u32;
                self.0[i] |= bit;
            }
        }
    }

    /// Returns the occupied height of lane `lane`, or `0` if it is empty.
    #[inline]
    #[must_use]
    pub fn height(&self, lane: usize) -> i32 {
        let bits = self.0[lane];
        if bits != 0 {
            let idx = 64 - 1 - bits.leading_zeros() as i32;
            return idx / WIDTH + 1;
        }

        0
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
