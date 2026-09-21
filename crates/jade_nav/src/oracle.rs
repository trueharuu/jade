use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;

use crate::buffer::Moves;
use crate::op::apply_rotation;
use crate::op::check;
use crate::op::fit_map;
use crate::op::usable_map;
use crate::queue::Queue;

/// BFS move generation for a single piece, returning all reachable
/// landed positions on the given board. A placement is reported only when
/// every one of its cells lands in the play field (the bottom `PLAY_LINES`
/// rows), over which the board can be filled.
#[inline(always)]
#[must_use]
pub fn generate<const P: Piece>(board: &Board) -> Moves {
    let mut queue = Queue::new();
    let mut visited: Moves = Moves::empty(P);
    let mut landed = Moves::empty(P);

    let usable = usable_map::<P>(board);
    let fit = fit_map::<P>();

    let sx = 4;
    let sy = 5 - P.h_spawn();

    let spawn = Move::new(P, sx, sy, Rotation::North);
    if check::<P>(&usable, spawn.x(), spawn.y(), spawn.rotation() as usize) {
        queue.push_back(spawn);
    }

    while let Some(ghost) = queue.pop_front() {
        if !visited.insert(ghost) {
            continue;
        }

        let x = ghost.x();
        let y = ghost.y();
        let r = ghost.rotation() as usize;

        // hard drop
        let mut drop_y = y;
        while check::<P>(&usable, x, drop_y - 1, r) {
            drop_y -= 1;
        }

        {
            let dropped_ghost = Move::new(P, x, drop_y, ghost.rotation());
            let c = dropped_ghost.canonicalize();
            if fit[c.rotation() as usize].get(c.x(), c.y()) {
                let _ = landed.insert(c);
            }
        }

        // lateral movement
        'd: for dx in [-1, 1] {
            let x1 = x + dx;
            if !check::<P>(&usable, x1, y, r) {
                continue 'd;
            }

            let new_ghost = Move::new(ghost.piece(), x1, y, ghost.rotation());
            queue.push_back(new_ghost);
        }

        // soft drop
        {
            let mut dy = y;
            if check::<P>(&usable, x, dy - 1, r) {
                dy -= 1;
            }

            if dy != y {
                let new_ghost = Move::new(ghost.piece(), x, dy, ghost.rotation());
                queue.push_back(new_ghost);
            }
        }

        // rotation (cw)
        {
            let new_ghost = apply_rotation::<P>(&usable, &ghost, ghost.rotation().cw());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }

        // rotation (ccw)
        {
            let new_ghost = apply_rotation::<P>(&usable, &ghost, ghost.rotation().ccw());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }

        // rotation (180)
        {
            let new_ghost = apply_rotation::<P>(&usable, &ghost, ghost.rotation().flip());
            if new_ghost != ghost {
                queue.push_back(new_ghost);
            }
        }
    }

    landed
}
