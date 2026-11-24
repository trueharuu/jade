/// A value produced by the decision search.
///
/// `u64` is the first implementation. Future score-based implementations can
/// provide their own type.
pub trait Value: Copy + Send + Sync + PartialEq + PartialOrd + 'static {
    fn zero() -> Self;
    fn one() -> Self;
    fn epsilon() -> Self;

    #[must_use]
    fn add(self, rhs: Self) -> Self;
    
    fn better(self, other: Self) -> bool;
    fn is_zero(self) -> bool {
        self == Self::zero()
    }

    #[must_use]
    fn mul_small(self, value: usize) -> Self;
}

impl Value for u64 {
    #[inline]
    fn zero() -> Self {
        0
    }
    #[inline]
    fn one() -> Self {
        1
    }
    #[inline]
    fn epsilon() -> Self {
        1
    }
    #[inline]
    fn add(self, rhs: Self) -> Self {
        self + rhs
    }
    #[inline]
    fn better(self, other: Self) -> bool {
        self < other
    }
    #[inline]
    fn mul_small(self, value: usize) -> Self {
        self * value as u64
    }
}