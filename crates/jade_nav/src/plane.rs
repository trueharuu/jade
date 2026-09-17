use std::cmp::Ordering;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not,
};
use std::simd::num::SimdUint;
use std::simd::Simd;

use crate::header::{dx_mask, MASK};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{LINES, WIDTH};

    #[test]
    fn shifted_moves_bits_to_expected_cells() {
        for x in 0..WIDTH {
            for y in 0..LINES {
                for dx in -3..=3 {
                    for dy in -4..=4 {
                        let bit = 1u64 << (y * WIDTH + x) as u32;
                        let out = Plane::<1>::from_array([bit]).shifted(dx, dy);
                        let out = out.0[0];

                        let nx = x + dx;
                        let ny = y + dy;
                        let expected = if (0..WIDTH).contains(&nx) && (0..LINES).contains(&ny) {
                            let mut w = 0u64;
                            if ny >= 0 && nx >= 0 {
                                w |= 1u64 << (ny * WIDTH + nx) as u32;
                            }
                            w
                        } else {
                            0
                        };
                        assert_eq!(
                            out,
                            expected,
                            "x={x} y={y} dx={dx} dy={dy} in={bit:b} out={out:b}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn any_and_popcount() {
        assert!(!Plane::<4>::empty().any());
        assert_eq!(Plane::<4>::empty().popcount(), 0);

        let p = Plane::<4>::from_array([0b101, 0, 0b1_0000, 0b111]);
        assert!(p.any());
        assert_eq!(p.popcount(), 6);
    }

    #[test]
    fn not_is_masked_to_sixty_bits() {
        let p = Plane::<2>::empty();
        assert_eq!((!p).to_array(), [MASK, MASK]);
    }
}