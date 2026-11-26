use jade_core::board::Board;
use jade_core::data::KICKS_I;
use jade_core::data::KICKS_TJLSZ;
use jade_core::header::LANDED_OK;
use jade_core::piece::Piece;
use jade_core::rot_idx;
use jade_core::rotation::Rotation;

use crate::buffer::Moves;
use crate::op::landable_map;
use crate::op::t_corners;
use crate::op::usable_map;
use crate::unroll;

#[inline]
#[must_use]
pub fn generate<const P: Piece, const SPINS: bool>(board: &Board) -> Moves {
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

    // Spin tracking: cells reachable with each spin stratum. Movement resets
    // spin to `None`, so shifted cells are recorded in `none_spin`; rotation
    // arrivals are recorded according to `label` (corner + kick index).
    let mut none_spin = [Board::empty(); Rotation::NB];
    let mut mini_spin = [Board::empty(); Rotation::NB];
    let mut full_spin = [Board::empty(); Rotation::NB];

    let is_t = matches!(P, Piece::T);
    let tc = if is_t && SPINS {
        Some(t_corners(board))
    } else {
        None
    };

    // Single-pass BFS in the shape of Cobra's movegen.
    let mut search = [Board::empty(); Rotation::NB];
    let mut unsearched = [Board::empty(); Rotation::NB];

    unroll!(r, ss, { unsearched[r] = fit[r]; });

    search[0].set(sx, sy);
    unsearched[0].0 &= !(1u64 << (sy * 10 + sx));
    none_spin[0].set(sx, sy);

    let mut pending = 1u32;

    while pending != 0 {
        unroll!(r, ss, {
            if pending & (1 << r) != 0 {
                pending &= !(1 << r);

                // Shifts reset spin: newly reached cells are `None`.
                shifts(&mut search, &mut unsearched, r, &fit, &mut none_spin);

                if !matches!(P, Piece::O) {
                    kick_seq::<P, { r }, SPINS>(
                        &mut search,
                        &mut unsearched,
                        &fit,
                        &mut pending,
                        tc.as_ref(),
                        &mut none_spin,
                        &mut mini_spin,
                        &mut full_spin,
                    );
                }

                search[r] = Board::empty();
            }
        });
    }

    let mut moves = Moves::empty(P);
    unroll!(r, cs, {
        let reached = !unsearched[r] & fit[r];
        let landed_mask = landed[r] & Board(LANDED_OK[P as usize][r]);
        let landed_cells = reached & landed_mask;
        // Spin is kept only when the placement is already on support. A piece
        // that has to fall to land loses its spin and counts as `None`.
        let floating = reached & !landed[r];
        let dropped_none = project_down(floating, landed[r]) & Board(LANDED_OK[P as usize][r]);

        moves.none[r] = (landed_cells & none_spin[r]) | dropped_none;
        if is_t && SPINS {
            moves.mini[r] = landed_cells & mini_spin[r];
            moves.full[r] = landed_cells & full_spin[r];
        }
    });

    moves
}

#[inline]
fn shifts(
    search: &mut [Board; Rotation::NB],
    unsearched: &mut [Board; Rotation::NB],
    r: usize,
    fit: &[Board; Rotation::NB],
    none_spin: &mut [Board; Rotation::NB],
) {
    loop {
        let g = search[r];
        let all = g.shr() | g.shl() | g.shd();
        none_spin[r] |= all & fit[r];
        let temp = all & unsearched[r];

        if !temp.any() {
            break;
        }

        search[r] |= temp;
        unsearched[r] &= !temp;
    }
}

#[inline]
const fn project_down(sources: Board, landable: Board) -> Board {
    let mut result = Board::empty();
    let mut x = 0;
    while x < 10 {
        let mut src_col = 0u8;
        let mut land_col = 0u8;
        let mut y = 0;
        while y < 6 {
            if sources.0 & (1u64 << (y * 10 + x)) != 0 {
                src_col |= 1 << y;
            }
            if landable.0 & (1u64 << (y * 10 + x)) != 0 {
                land_col |= 1 << y;
            }
            y += 1;
        }

        // For each source bit, find the first landable bit at or below it.
        let mut yy = 0;
        while yy < 6 {
            if src_col & (1 << yy) != 0 {
                // find nearest landable at or below yy
                let mut target = -1i32;
                let mut t = yy;
                while t >= 0 {
                    if land_col & (1 << t) != 0 {
                        target = t;
                        break;
                    }
                    t -= 1;
                }
                if target >= 0 {
                    result.0 |= 1u64 << (target as usize * 10 + x);
                }
            }
            yy += 1;
        }
        x += 1;
    }
    result
}

#[inline]
fn kick_seq<const P: Piece, const R: usize, const SPINS: bool>(
    search: &mut [Board; Rotation::NB],
    unsearched: &mut [Board; Rotation::NB],
    fit: &[Board; Rotation::NB],
    pending: &mut u32,
    tc: Option<&(Board, [Board; Rotation::NB])>,
    none_spin: &mut [Board; Rotation::NB],
    mini_spin: &mut [Board; Rotation::NB],
    full_spin: &mut [Board; Rotation::NB],
) {
    let kt = match P {
        Piece::I => &KICKS_I,
        _ => &KICKS_TJLSZ,
    };

    let is_t = matches!(P, Piece::T);

    unroll!(t, P.search_size(), {
        let lane = kt[R][t];
        let mut temp = search[R];
        let mut result = Board::empty();

        let mut n = 0usize;
        while n < lane.1 {
            let kx = i32::from(lane.0[n].0);
            let ky = i32::from(lane.0[n].1);

            let arrivals = temp.shifted(kx, ky) & fit[t];

            // Label spin arrivals (T only), for all first-valid arrivals,
            // regardless of whether the cell is new to `unsearched`.
            if is_t && SPINS {
                if let Some((c3, cf)) = tc {
                    let spins = arrivals & *c3;
                    if n >= 4 {
                        full_spin[t] |= spins;
                    } else {
                        let front = spins & cf[t];
                        let back = spins & !cf[t];
                        full_spin[t] |= front;
                        mini_spin[t] |= back;
                    }
                    // Rotations that do not satisfy 3 corners carry no spin.
                    let no_spin_arrivals = arrivals & !*c3;
                    none_spin[t] |= no_spin_arrivals;
                }
            } else {
                none_spin[t] |= arrivals;
            }

            result |= arrivals;
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