use crate::header::{TLINES, WIDTH};
use crate::{piece::Piece, rotation::Rotation};

/// Rotates an `(x, y)` cell offset by a canonical quarter-turn index.
#[inline]
#[must_use]
pub const fn rot_cell(c: (i8, i8), rot: usize) -> (i8, i8) {
    match rot & 3 {
        0 => c,
        1 => (c.1, -c.0),
        2 => (-c.0, -c.1),
        _ => (-c.1, c.0),
    }
}

/// Canonical rotated cell offsets for each [`Piece`] and [`Rotation`].
pub const CELLS: [[[(i8, i8); 3]; Rotation::NB]; Piece::NB] = const {
    let mut out = [[[(0, 0); _]; _]; _];
    let mut p = 0;

    while p < Piece::NB {
        let base = Piece::from_u8(p as u8).unwrap().base_cells();

        let mut r = 0;
        while r < Rotation::NB {
            let mut i = 0;
            while i < 3 {
                out[p][r][i] = rot_cell(base[i], r);
                i += 1;
            }

            r += 1;
        }

        p += 1;
    }

    out
};

/// Compact place-mask `(word, x_bias)` for a single band.
///
/// `word` places the piece with its anchor cell at row `yr` of the band, and
/// its leftmost cell at column 0. `x_bias` is the anchor cell's column
/// relative to the leftmost cell.
///
/// A placement never crosses a band boundary, so the whole piece must fit in
/// the band. Entries whose cells would fall outside the band are zeroed and
/// unused.
pub type PlaceMask = (u64, i8);

/// Full table of precomputed place masks indexed by piece, canonical
/// rotation, and anchor row within the band.
pub type PlaceMaskTable = [[[PlaceMask; TLINES as usize]; Rotation::NB]; Piece::NB];

/// Precomputed placement mask table.
pub const PMASK: PlaceMaskTable = const {
    let mut out = [[[(0u64, 0i8); TLINES as usize]; Rotation::NB]; Piece::NB];
    let mut p = 0;
    while p < Piece::NB {
        let rc_max = Piece::ALL[p].groups();
        let mut rc = 0;
        while rc < rc_max {
            let cells = [
                (0i8, 0i8),
                CELLS[p][rc][0],
                CELLS[p][rc][1],
                CELLS[p][rc][2],
            ];
            let mut dx = 0i32;
            let mut i = 0;
            while i < 4 {
                if (cells[i].0 as i32) < dx {
                    dx = cells[i].0 as i32;
                }
                i += 1;
            }

            let xb = -dx;
            let mut yr = 0usize;
            while yr < TLINES as usize {
                let mut word = 0u64;
                let mut fits = true;
                let mut i = 0;
                while i < 4 {
                    let row = yr as i32 + cells[i].1 as i32;
                    if row < 0 || row >= TLINES as i32 {
                        fits = false;
                    } else {
                        let c = cells[i].0 as i32 + xb;
                        word |= 1u64 << (row as u32 * WIDTH + c as u32);
                    }
                    i += 1;
                }
                out[p][rc][yr] = (if fits { word } else { 0 }, xb as i8);
                yr += 1;
            }
            rc += 1;
        }
        p += 1;
    }
    out
};

#[cfg(test)]
mod tests {
    use super::{CELLS, PMASK};
    use crate::header::{TLINES, WIDTH};
    use crate::piece::Piece;

    #[test]
    fn pmask_matches_brute_placement() {
        for p in 0..Piece::NB {
            for rc in 0..Piece::ALL[p].groups() {
                let cells = [
                    (0i8, 0i8),
                    CELLS[p][rc][0],
                    CELLS[p][rc][1],
                    CELLS[p][rc][2],
                ];
                let mut dx = 0i32;
                let mut i = 0;
                while i < 4 {
                    if (cells[i].0 as i32) < dx {
                        dx = cells[i].0 as i32;
                    }
                    i += 1;
                }
                let xb = -dx;

                for (yr, &(word, xb_word)) in PMASK[p][rc].iter().enumerate() {
                    let mut fits = true;
                    let mut expected = 0u64;
                    for (cx, cy) in cells {
                        let row = yr as i32 + cy as i32;
                        if row < 0 || row >= TLINES as i32 {
                            fits = false;
                        } else {
                            let c = cx as i32 + xb;
                            expected |= 1u64 << (row as u32 * WIDTH + c as u32);
                        }
                    }
                    let expected_word = if fits { expected } else { 0 };
                    let expected = (expected_word, xb as i8);
                    assert_eq!((word, xb_word), expected, "{p} {rc} {yr}");
                }
            }
        }
    }
}