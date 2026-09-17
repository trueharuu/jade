use std::{
    cmp::Ordering,
    simd::{prelude::SimdPartialEq, Mask, Simd},
};

use crate::header::{self, FULL_MASK, WIDTH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct Batch<const N: usize>(pub Simd<u64, N>);

impl<const N: usize> Batch<N> {
    /// An empty batch of `N` boards.
    #[inline]
    #[must_use]
    pub fn empty() -> Self {
        Self(Simd::splat(0))
    }

    /// A batch with every lane holding the same board.
    #[inline]
    #[must_use]
    pub fn from_board(field: u64) -> Self {
        Self(Simd::splat(field))
    }

    /// Loads one distinct board per lane.
    #[inline]
    #[must_use]
    pub fn from_array(boards: [u64; N]) -> Self {
        Self(Simd::from_array(boards))
    }

    /// Extracts every lane.
    #[inline]
    #[must_use]
    pub fn to_array(self) -> [u64; N] {
        self.0.to_array()
    }

    /// Places the given mask into every lane with XOR.
    ///
    /// Callers must test `overlaps` first; XOR with overlapping bits
    /// would corrupt the board.
    #[inline]
    #[must_use]
    pub fn place(self, mask: Simd<u64, N>) -> Self {
        Self(self.0 ^ mask)
    }

    /// Returns `true` per lane where the board and the mask share bits.
    #[inline]
    #[must_use]
    pub fn overlaps(self, mask: Simd<u64, N>) -> Mask<i64, N> {
        (self.0 & mask).simd_ne(Simd::splat(0))
    }

    /// Returns `true` per lane where all 4 lines are full.
    #[inline]
    #[must_use]
    pub fn is_full(self) -> Mask<i64, N> {
        self.0.simd_eq(Simd::splat(FULL_MASK))
    }

    /// Returns `true` per lane where the board is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(self) -> Mask<i64, N> {
        self.0.simd_eq(Simd::splat(0))
    }

    /// Shifts every board in the batch by the same `(dx, dy)` offset.
    ///
    /// `dy > 0` shifts up whilst `dy < 0` shifts down. `dx > 0` shifts right whilst `dx < 0` shifts left. Bits shifted past the top, bottom, left, or right edges are dropped.
    #[inline]
    #[must_use]
    pub fn shifted(&self, dx: i32, dy: i32) -> Self {
        let dy_shift = dy * WIDTH;

        let v_shift = match dy_shift.cmp(&0) {
            Ordering::Equal => self.0,
            Ordering::Greater => self.0 << Simd::splat(dy_shift as u64),
            Ordering::Less => self.0 >> Simd::splat((-dy_shift) as u64),
        };

        let result = match dx.cmp(&0) {
            Ordering::Equal => v_shift,
            Ordering::Greater => v_shift << Simd::splat(dx as u64),
            Ordering::Less => v_shift >> Simd::splat((-dx) as u64),
        } & Simd::splat(header::dx_mask(dx));

        Self(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_rng(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    #[test]
    fn batched_ops_match_scalar_lanes() {
        let mut state = 0x1234_5678_9abc_def0u64;
        for _ in 0..256 {
            let boards: [u64; 8] = std::array::from_fn(|_| next_rng(&mut state) & FULL_MASK);
            let masks: [u64; 8] = std::array::from_fn(|_| next_rng(&mut state) & FULL_MASK);

            let batch: Batch<8> = Batch::from_array(boards);
            let mask: Simd<u64, 8> = Simd::from_array(masks);

            let placed = batch.place(mask).to_array();
            let overlap = batch.overlaps(mask).to_array();
            let is_full = batch.is_full().to_array();
            let is_empty = batch.is_empty().to_array();

            for i in 0..8 {
                let scalar = Batch::<1>::from_board(boards[i]);
                assert_eq!(placed[i], boards[i] ^ masks[i]);
                assert_eq!(overlap[i], (boards[i] & masks[i]) != 0);
                assert_eq!(is_full[i], scalar.is_full().to_array()[0]);
                assert_eq!(is_empty[i], scalar.is_empty().to_array()[0]);
            }
        }
    }

    #[test]
    fn place_roundtrip() {
        let boards = [0b1000_0000u64, 0, FULL_MASK, FULL_MASK];
        let batch = Batch::<4>::from_array(boards);
        let placed = batch.place(Simd::from_array([0b1000_0000u64, 0b10, 0, 0]));
        assert_eq!(placed.to_array(), [0u64, 0b10, FULL_MASK, FULL_MASK]);
    }

    #[test]
    fn full_and_empty_detection() {
        let batch = Batch::<4>::from_array([FULL_MASK, 0, 1, FULL_MASK]);
        assert_eq!(batch.is_full().to_array(), [true, false, false, true]);
        assert_eq!(batch.is_empty().to_array(), [false, true, false, false]);
        assert_eq!(Batch::<4>::empty().is_empty().to_array(), [true; 4]);
    }
}
