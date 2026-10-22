use jade_core::board::Board;
use jade_core::data::KICKS_I;
use jade_core::data::KICKS_TJLSZ;
use jade_core::header::LANDED_OK;
use jade_core::piece::Piece;
use jade_core::rot_idx;
use jade_core::rotation::Rotation;

use crate::buffer::Moves;
use crate::op::landable_map;
use crate::op::usable_map;
use crate::unroll;

#[inline]
#[must_use]
pub fn generate<const P: Piece>(board: &Board) -> Moves {
    let usable = usable_map::<P>(board);
    let cs = P.groups();
    let ss = P.search_size();
    let landed = landable_map(&usable, cs);

    let mut fit = [Board::empty(); Rotation::NB];

    unroll!(r, Rotation::NB, {
        let (ox, oy) = P.canonical_offset(rot_idx!(r));
        let rc: usize = P.canonical_rotation(rot_idx!(r)) as usize;
        fit[r] = usable[rc].shifted(ox, oy);
    });

    let sx = 4;
    let sy = P.h_spawn();

    if !usable[0].get(sx, sy) {
        return Moves::empty(P);
    }

    // Single-pass BFS in the shape of Cobra's movegen. `search[r]` holds
    // reached cells of raw rotation `r` whose shifts and kicks are still
    // pending; `unsearched[r]` holds cells of `fit[r]` not reached yet.
    // Each processed rotation runs its shifts to closure, expands its kick
    // lanes once, then clears its plane. `pending` requeues rotations that
    // received new cells.
    //
    // Note: Cobra additionally seeds every rotation with the fall closure
    // from the board top before this loop, which is exact under their
    // drop-from-top entry and is inexact under the spawn-entry oracle this
    // module must match, so seeding here starts from spawn only.
    let mut search = [Board::empty(); Rotation::NB];
    let mut unsearched = [Board::empty(); Rotation::NB];

    unroll!(r, ss, { unsearched[r] = fit[r]; });

    search[0].set(sx, sy);
    unsearched[0].0 &= !(1u64 << (sy * 10 + sx));

    let mut pending = 1u32;

    while pending != 0 {
        unroll!(r, ss, {
            if pending & (1 << r) != 0 {
                pending &= !(1 << r);

                shifts(&mut search, &mut unsearched, r);

                if !matches!(P, Piece::O) {
                    kick_seq::<P, { r }>(&mut search, &mut unsearched, &fit, &mut pending);
                }

                search[r] = Board::empty();
            }
        });
    }

    let mut moves = Moves::empty(P);
    unroll!(r, cs, {
        // reached cells are the complement of `unsearched` inside `fit`;
        // landed_ok only exists to remove technically valid placements but exceed the wanted 10x4 area
        moves.mask[r] = (!unsearched[r] & fit[r]) & landed[r] & Board(LANDED_OK[P as usize][r]);
    });

    moves
}

#[inline]
fn shifts(search: &mut [Board; Rotation::NB], unsearched: &mut [Board; Rotation::NB], r: usize) {
    loop {
        let g = search[r];
        let temp = (g.shr() | g.shl() | g.shd()) & unsearched[r];

        if !temp.any() {
            break;
        }

        search[r] |= temp;
        unsearched[r] &= !temp;
    }
}

#[inline]
fn kick_seq<const P: Piece, const R: usize>(
    search: &mut [Board; Rotation::NB],
    unsearched: &mut [Board; Rotation::NB],
    fit: &[Board; Rotation::NB],
    pending: &mut u32,
) {
    let kt = match P {
        Piece::I => &KICKS_I,
        _ => &KICKS_TJLSZ,
    };

    unroll!(t, P.search_size(), {
        let lane = kt[R][t];
        let mut temp = search[R];
        let mut result = Board::empty();

        let mut n = 0;
        while n < lane.1 {
            let kx = i32::from(lane.0[n].0);
            let ky = i32::from(lane.0[n].1);

            // candidates are accumulated against the static `fit` board, and
            // every source whose target fits is dropped, so the first-valid
            // kick order is kept regardless of which sources were visited
            result |= temp.shifted(kx, ky) & fit[t];
            temp &= !fit[t].shifted(-kx, -ky);

            n += 1;
        }

        let result = result & unsearched[t];

        if result.any() {
            search[t] |= result;
            unsearched[t] &= !result;
            *pending |= 1 << t;
        }
    });
}
