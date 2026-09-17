//! Move buffers produced by move generation.

use std::simd::Simd;

use jade_core::header::WIDTH;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;

use crate::header::LINES;
use crate::plane::Plane;

/// Result of move generation.
///
/// Holds the reachable locked-origin positions, one plane per
/// rotation. Each lane of a plane is one independent working field; the
/// lane-less fields are all empty. jade has no spins, so there are no
/// spin buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moves<const N: usize> {
    /// The piece for which these moves were generated.
    pub piece: Piece,
    /// Locked-origin positions for each rotation, one plane per lane.
    pub per_rot: [Plane<N>; Rotation::NB],
}

impl<const N: usize> Moves<N> {
    /// Creates a new, empty `Moves` result for the given piece.
    #[inline]
    #[must_use]
    pub const fn empty(piece: Piece) -> Self {
        Self {
            piece,
            per_rot: [Plane::empty(); Rotation::NB],
        }
    }

    /// Inserts `mv` in lane `lane`.
    /// Returns `true` when the move was not already present, `false`
    /// otherwise.
    #[inline]
    #[must_use]
    pub fn insert(&mut self, lane: usize, mv: Move) -> bool {
        let r = mv.rotation() as usize;
        let word = if r < Rotation::NB { &mut self.per_rot[r] } else { return false };

        let mut lanes = word.0.to_array();
        let idx = mv.y() * WIDTH + mv.x();
        debug_assert!((0..60).contains(&idx), "insert overflow idx={idx} mv={mv:?}");
        let bit = 1u64 << idx as u32;

        if lanes[lane] & bit != 0 {
            return false;
        }

        lanes[lane] |= bit;
        word.0 = Simd::from_array(lanes);
        true
    }

    /// Returns `true` when the buffer contains `mv` in lane `lane`.
    #[inline]
    #[must_use]
    pub fn contains(&self, lane: usize, mv: Move) -> bool {
        let r = mv.rotation() as usize;
        if r >= Rotation::NB {
            return false;
        }

        let bit = 1u64 << (mv.y() * WIDTH + mv.x()) as u32;
        self.per_rot[r].0[lane] & bit != 0
    }

    /// Returns an iterator over all moves stored in this buffer, in
    /// ascending `(lane, rotation, y, x)` order.
    #[inline]
    #[must_use]
    #[allow(clippy::iter_not_returning_iterator)]
    pub const fn iter(&self) -> MovesIter<'_, N> {
        MovesIter {
            moves: self,
            lane: 0,
            rotation: 0,
            x: 0,
            y: 0,
        }
    }

    /// Returns the total number of moves in the buffer, summed over all
    /// lanes.
    #[inline]
    #[must_use]
    pub fn popcount(&self) -> u64 {
        let mut total = 0;
        for r in 0..Rotation::NB {
            total += self.per_rot[r].popcount();
        }
        total
    }
}

/// An iterator over the moves in a [`Moves`] buffer.
pub struct MovesIter<'a, const N: usize> {
    moves: &'a Moves<N>,
    lane: usize,
    rotation: usize,
    x: i32,
    y: i32,
}

impl<const N: usize> Iterator for MovesIter<'_, N> {
    type Item = (usize, Move);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.lane < N {
            while self.rotation < Rotation::NB {
                let plane = self.moves.per_rot[self.rotation];
                while self.y < LINES {
                    while self.x < WIDTH {
                        let bit = 1u64 << (self.y * WIDTH + self.x) as u32;
                        if plane.0[self.lane] & bit != 0 {
                            let mv = Move::new(
                                self.moves.piece,
                                Rotation::from_u8(self.rotation as u8),
                                self.x,
                                self.y,
                            );
                            self.x += 1;
                            return Some((self.lane, mv));
                        }
                        self.x += 1;
                    }
                    self.x = 0;
                    self.y += 1;
                }
                self.y = 0;
                self.rotation += 1;
            }
            self.rotation = 0;
            self.lane += 1;
        }
        None
    }
}

impl<'a, const N: usize> IntoIterator for &'a Moves<N> {
    type Item = (usize, Move);
    type IntoIter = MovesIter<'a, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_dedups_per_lane() {
        let mut m = Moves::<2>::empty(Piece::T);
        let a = Move::new(Piece::T, Rotation::North, 5, 2);
        let b = Move::new(Piece::T, Rotation::North, 5, 2);

        assert!(m.insert(0, a));
        assert!(!m.insert(0, b));
        assert!(m.insert(1, b));
        assert!(m.contains(0, a));
        assert!(m.contains(1, b));
        assert_eq!(m.popcount(), 2);
    }

    #[test]
    fn different_rotations_are_distinct() {
        let mut m = Moves::<1>::empty(Piece::T);
        let n = Move::new(Piece::T, Rotation::North, 5, 2);
        let e = Move::new(Piece::T, Rotation::East, 5, 2);

        assert!(m.insert(0, n));
        assert!(m.insert(0, e));
        assert_eq!(m.popcount(), 2);
        assert!(m.contains(0, n));
        assert!(m.contains(0, e));
    }

    #[test]
    fn iter_yields_all_lanes_in_order() {
        let mut m = Moves::<2>::empty(Piece::I);
        let a = Move::new(Piece::I, Rotation::North, 7, 3);
        let b = Move::new(Piece::I, Rotation::East, 1, 0);
        let c = Move::new(Piece::I, Rotation::North, 2, 1);
        assert!(m.insert(0, a));
        assert!(m.insert(0, b));
        assert!(m.insert(1, c));

        let got: Vec<_> = m.iter().collect();
        assert_eq!(got, vec![(0, a), (0, b), (1, c)]);
    }
}
