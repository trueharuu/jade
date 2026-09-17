use std::{cmp::Ordering, simd::Simd};

use crate::header::{self, WIDTH};

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
