//! Scalar BFS reference for move generation.
//!
//! This is a correctness oracle. The fast generator in `fast.rs` must
//! match it exactly. The oracle follows rue's algorithm, adapted to
//! jade: one independent field per lane, a 6-row plane, and a lock rule
//! that keeps every locked cell inside the 4-line storable field.

use std::mem::MaybeUninit;

use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;

use crate::movegen::buffer::Moves;
use crate::movegen::op::apply_rotation;
use crate::movegen::op::check_fast;
use crate::movegen::op::usable_map;
use crate::header::LINES;
use crate::plane::Plane;

/// Fixed-capacity ring queue for BFS states.
const CAP: usize = 4096;
const QUEUE_MASK: usize = CAP - 1;

struct Queue<T> {
    buf: [MaybeUninit<T>; CAP],
    front: usize,
    back: usize,
}

impl<T> Queue<T> {
    #[inline]
    fn new() -> Self {
        Self {
            // SAFETY: An uninitialized `[MaybeUninit<_>; CAP]` is valid.
            buf: unsafe { MaybeUninit::uninit().assume_init() },
            front: 0,
            back: 0,
        }
    }

    #[inline]
    fn push_back(&mut self, val: T) {
        assert!(self.back - self.front != CAP, "queue full");
        self.buf[self.back & QUEUE_MASK].write(val);
        self.back = self.back.wrapping_add(1);
    }

    #[inline]
    fn pop_front(&mut self) -> Option<T> {
        if self.front == self.back {
            return None;
        }
        let val = unsafe { self.buf[self.front & QUEUE_MASK].assume_init_read() };
        self.front = self.front.wrapping_add(1);
        Some(val)
    }
}

/// BFS move generation for a single piece on a single field.
///
/// Returns the landed-origin planes and the total lock count.
#[inline]
#[must_use]
pub fn generate<const P: Piece>(board: u64, y: i32, force: i32) -> (Moves<1>, u64) {
    let mut queue = Queue::<Move>::new();
    let mut visited = Moves::<1>::empty(P);
    let mut landed = Moves::<1>::empty(P);

    let plane = Plane::<1>::from_array([board]);
    let usable = usable_map::<1, P>(plane);

    // `y` is the drop height (the top of the stack).
    // When it rises within `h_spawn` rows of the spawn row, a piece
    // spawned during play overlaps the stack. The spawn then rises to
    // the first free cell in the spawn column. The scan is bounded by
    // `force` rows of allowed rise plus the plane top. If the bound has
    // no free cell, the piece is locked out and no placement is
    // reachable.
    let sx = 4;
    let sy = 4;

    let mut spawn_row = sy;
    if y > sy - P.h_spawn() {
        let threshold = (sy + force + 1).min(LINES);
        while spawn_row < threshold
            && !check_fast::<P>(&usable, sx, spawn_row, Rotation::North as usize)
        {
            spawn_row += 1;
        }

        if spawn_row == threshold {
            return (landed, 0);
        }
    }

    let spawn = Move::new(P, Rotation::North, sx, spawn_row);
    if check_fast::<P>(&usable, spawn.x(), spawn.y(), spawn.rotation() as usize) {
        queue.push_back(spawn);
    }
    while let Some(ghost) = queue.pop_front() {
        if !visited.insert(0, ghost) {
            continue;
        }

        let x = ghost.x();
        let y = ghost.y();
        let rotation = ghost.rotation();

        // hard drop
        let mut drop_y = y;
        while check_fast::<P>(&usable, x, drop_y - 1, rotation as usize) {
            drop_y -= 1;
        }

        let dropped = Move::new(P, rotation, x, drop_y);
        if dropped.canonicalize().mask().is_some() {
            let _ = landed.insert(0, dropped.canonicalize());
        }

        // extended lateral movement (das)
        'd: for dx in [-1, 1] {
            let mut x1 = x;
            loop {
                let next = x1 + dx;
                if !check_fast::<P>(&usable, next, y, rotation as usize) {
                    break;
                }
                x1 = next;
            }

            if x1 == x {
                continue 'd;
            }

            let new_ghost = Move::new(P, rotation, x1, y);
            queue.push_back(new_ghost);
        }

        // lateral movement
        for dx in [-1, 1] {
            let x1 = x + dx;
            if !check_fast::<P>(&usable, x1, y, rotation as usize) {
                continue;
            }

            queue.push_back(Move::new(P, rotation, x1, y));
        }

        // soft drop (infinite)
        {
            let mut dy = y;
            while check_fast::<P>(&usable, x, dy - 1, rotation as usize) {
                dy -= 1;
            }

            if dy != y {
                queue.push_back(Move::new(P, rotation, x, dy));
            }
        }

        // rotation (cw)
        {
            let new_ghost = apply_rotation::<P>(&plane, &usable, &ghost, rotation.cw());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }

        // rotation (ccw)
        {
            let new_ghost = apply_rotation::<P>(&plane, &usable, &ghost, rotation.ccw());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }

        // rotation (180)
        {
            let new_ghost = apply_rotation::<P>(&plane, &usable, &ghost, rotation.cw().cw());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }
    }

    (landed, landed.popcount())
}

/// All canonical locked-origin moves reachable for `P` on `board`.
#[inline]
#[must_use]
pub fn movegen<const P: Piece>(board: u64, y: i32, force: i32) -> Vec<Move> {
    generate::<P>(board, y, force).0.iter().map(|(_, mv)| mv).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lock_fits_the_field() {
        macro_rules! each {
            ($piece:ident) => {{
                let piece = Piece::$piece;
                let (moves, count) = generate::<{ Piece::$piece }>(0, 0, 0);
                assert!(count > 0, "piece={piece}");
                for (_, mv) in moves.iter() {
                    assert!(mv.mask().is_some(), "piece={piece} mv={mv:?}");
                    assert_eq!(mv, mv.canonicalize(), "piece={piece} mv={mv:?}");
                }
            }};
        }
        each!(T);
        each!(I);
        each!(J);
        each!(L);
        each!(O);
        each!(S);
        each!(Z);
    }

    #[test]
    fn full_field_locks_out() {
        let full = (1u64 << 40) - 1;
        macro_rules! each {
            ($piece:ident) => {{
                let (_, count) = generate::<{ Piece::$piece }>(full, 0, 0);
                assert_eq!(count, 0, "piece={}", Piece::$piece);
            }};
        }
        each!(T);
        each!(I);
        each!(J);
        each!(L);
        each!(O);
        each!(S);
        each!(Z);
    }

    #[test]
    fn locks_never_overlap_field_cells() {
        let occ = 1u64 << (2 * 10 + 4);
        macro_rules! each {
            ($piece:ident) => {{
                let piece = Piece::$piece;
                let (moves, count) = generate::<{ Piece::$piece }>(occ, 0, 0);
                assert!(count > 0, "piece={piece}");
                for (_, mv) in moves.iter() {
                    assert!(
                        mv.mask().is_some() && (mv.mask().unwrap() & occ).count_ones() == 0,
                        "piece={piece} mv={mv:?}",
                    );
                }
            }};
        }
        each!(T);
        each!(I);
        each!(J);
        each!(L);
        each!(O);
        each!(S);
        each!(Z);
    }
}