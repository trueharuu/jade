use jade_core::board::Board;
use jade_core::data::KICKS_I;
use jade_core::data::KICKS_TJLSZ;
use jade_core::header::LANDED_OK;
use jade_core::piece::Piece;
use jade_core::rot_idx;
use jade_core::rotation::Rotation;

use crate::buffer::Moves;
use crate::op::usable_map;
use crate::unroll;

/// Exact move generation for a single piece on a 6-row plane.
///
/// Reachability is the fixed point of fall, lateral step, and SRS kicks
/// (cw, ccw, and 180) over four raw-rotation origin planes, started from
/// the North spawn origin. The raw rotations stay distinct, matching the
/// oracle state for state. Raw-rotation locks merge into canonical lock
/// planes at emission.
#[inline]
#[must_use]
pub fn generate<const P: Piece>(board: &Board) -> Moves {
    let usable = usable_map::<P>(board);
    let cs = P.groups();
    let ss = P.search_size();

    // Fit plane per raw rotation: the canonical usable plane shifted by
    // the raw rotation's canonical offset.
    let mut fit = [Board::empty(); Rotation::NB];
    unroll!(r, Rotation::NB, {
        let (ox, oy) = P.canonical_offset(rot_idx!(r));
        let rc: usize = P.canonical_rotation(rot_idx!(r)) as usize;
        fit[r] = usable[rc].shifted(ox, oy);
    });

    let sx = 4;
    let sy = P.h_spawn();

    // The oracle starts its BFS from the North spawn origin and reports
    // nothing when that origin is blocked, so fast must match.
    if !usable[0].get(sx, sy) {
        return Moves::empty(P);
    }

    let mut search = [Board::empty(); Rotation::NB];
    search[0].set(sx, sy);

    loop {
        let (before_0, before_1, before_2, before_3) = (search[0].0, search[1].0, search[2].0, search[3].0);

        unroll!(r, ss, {
            closure(&mut search, &fit, r);
        });

        if !matches!(P, Piece::O) {
            unroll!(r, ss, {
                kick_seq::<P>(&mut search, &fit, r);
            });
        }

        if before_0 == search[0].0 && before_1 == search[1].0 && before_2 == search[2].0 && before_3 == search[3].0 {
            break;
        }
    }

    // Merge raw-rotation reachability into canonical rotation planes.
    let mut locked = [Board::empty(); Rotation::NB];
    unroll!(r, cs, {
        let (ox, oy) = P.canonical_offset(rot_idx!(r));
        locked[r] = search[r].shifted(-ox, -oy);
    });

    if P.group2() {
        let s_off = P.canonical_offset(Rotation::South);
        let w_off = P.canonical_offset(Rotation::West);
        locked[0] |= search[2].shifted(-s_off.0, -s_off.1);
        locked[1] |= search[3].shifted(-w_off.0, -w_off.1);
    }

    let mut moves = Moves::empty(P);
    unroll!(r, cs, {
        let no_land = !usable[r].shifted(0, 1);
        moves.mask[r] =
            locked[r] & usable[r] & no_land & Board::new(LANDED_OK[P as usize][r]);
    });

    moves
}

/// Closes `search[r]` under lateral steps (left/right).
///
/// Soft drop is pre-computed above via `vertical_ceiling`, so `closure`
/// only needs to close under single-side lateral moves.
#[inline]
fn closure(search: &mut [Board; Rotation::NB], fit: &[Board; Rotation::NB], r: usize) {
    loop {
        let g = search[r];
        let growth = ((g.shr() | g.shl() | g.shd()) & fit[r]) & !g;

        if !growth.any() {
            break;
        }

        search[r] |= growth;
    }
}

/// Adds the first-valid kick targets of rotation `r` to each other raw
/// rotation plane.
///
/// An SRS rotation applies its kicks in order, and only the first kick
/// whose target fits takes effect. A kick therefore processes only the
/// sources that earlier kicks in the same lane missed.
#[inline]
fn kick_seq<const P: Piece>(
    search: &mut [Board; Rotation::NB],
    fit: &[Board; Rotation::NB],
    r: usize,
) {
    let kt = match P {
        Piece::I => &KICKS_I,
        _ => &KICKS_TJLSZ,
    };

    let mut t = 0;
    while t < Rotation::NB {
        let lane = kt[r][t];
        let mut temp = search[r];

        let mut n = 0;
        while n < lane.1 {
            let kx = i32::from(lane.0[n].0);
            let ky = i32::from(lane.0[n].1);

            let cand = temp.shifted(kx, ky) & fit[t];

            if cand.any() {
                search[t] |= cand;
                temp &= !fit[t].shifted(-kx, -ky);
            }

            n += 1;
        }

        t += 1;
    }
}