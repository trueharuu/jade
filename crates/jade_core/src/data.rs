use crate::{
    header::{LINES, WIDTH},
    piece::Piece,
    rotation::Rotation,
};

/// Rotates a cell offset by a quarter-turn count. Ported unchanged: a
/// rotation matrix has nothing to do with board representation.
#[must_use]
pub const fn rot_cell(c: (i8, i8), rot: usize) -> (i8, i8) {
    match rot & 3 {
        0 => c,
        1 => (c.1, -c.0),
        2 => (-c.0, -c.1),
        _ => (-c.1, c.0),
    }
}

pub type Cells = [(i8, i8); 3];

pub const CELLS: [[Cells; Rotation::NB]; Piece::NB] = const {
    let mut out = [[[(0, 0); 3]; Rotation::NB]; Piece::NB];
    let pieces = Piece::ALL;

    let mut p = 0;
    while p < Piece::NB {
        let base = pieces[p].base_cells();
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

pub type PlaceMask = (u64, i8, i8, i8);

pub const PMASK: [[PlaceMask; Rotation::NB]; Piece::NB] = const {
    let mut out = [[(0u64, 0i8, 0i8, 0i8); Rotation::NB]; Piece::NB];
    let pieces = [
        Piece::T,
        Piece::I,
        Piece::J,
        Piece::L,
        Piece::O,
        Piece::S,
        Piece::Z,
    ];

    let mut p = 0;
    while p < Piece::NB {
        let piece = pieces[p];
        let mut rc = 0;
        while rc < piece.groups() {
            // The origin cell (0, 0) is implicit in `CELLS` (see its
            // definition above); add it back in here, the one place
            // this port needs the full four-cell shape.
            let three = CELLS[p][rc];
            let cells = [(0i8, 0i8), three[0], three[1], three[2]];

            let mut min_x = 0i32;
            let mut min_y = 0i32;
            let mut max_x = 0i32;
            let mut max_y = 0i32;
            let mut i = 0;
            while i < 4 {
                let x = cells[i].0 as i32;
                let y = cells[i].1 as i32;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
                i += 1;
            }

            let x_bias = -min_x;
            let height = max_y - min_y + 1;
            let width = max_x - min_x + 1;
            let mut mask = 0u64;
            let mut i = 0;
            while i < 4 {
                let x = cells[i].0 as i32 + x_bias;
                let y = cells[i].1 as i32 - min_y;
                mask |= 1u64 << (y * WIDTH + x) as u32;
                i += 1;
            }

            out[p][rc] = (mask, x_bias as i8, height as i8, width as i8);
            rc += 1;
        }
        p += 1;
    }
    out
};

#[must_use]
pub const fn place_mask(piece: Piece, rotation: Rotation, x: i32, y: i32) -> Option<u64> {
    let (ox, oy) = piece.canonical_offset(rotation);
    let canon = piece.canonical_rotation(rotation) as usize;
    let cx = x - ox;
    let cy = y - oy;

    let (mask, x_bias, height, width) = PMASK[piece as usize][canon];
    if cy < 0 || cy + height as i32 > LINES {
        return None;
    }
    let left = cx - x_bias as i32;
    if left < 0 || left + width as i32 > WIDTH {
        return None;
    }
    let shifted = mask << (cy * WIDTH) as u32;
    Some(shifted << left as u32)
}

/// A single kick wave for a rotation transition: up to 6 `(dx, dy)`
/// offsets to try in order, and how many of them are used.
pub type K6 = ([(i8, i8); 6], usize);
/// Kick table for every initial and final rotation pair.
pub type Kicks = [[K6; Rotation::NB]; Rotation::NB];

pub const NULL_KICK: K6 = ([(0, 0); 6], 0);

macro_rules! kick {
    ($k:expr, $i:expr, $j:expr, $(($x:expr, $y:expr))*) => {{
        #![allow(unused)]
        use Rotation::North as N;
        use Rotation::East as E;
        use Rotation::South as S;
        use Rotation::West as W;

        let mut kick = NULL_KICK;
        let mut k = 0;
        $(
            kick.0[k] = ($x, $y);
            k += 1;
        )*
        kick.1 = k;
        $k[$i as usize][$j as usize] = kick;
    }};
}

/// Kick table for T, J, L, S, and Z pieces.
pub const KICKS_TJLSZ: Kicks = {
    let mut out = [[NULL_KICK; Rotation::NB]; Rotation::NB];

    // T.NE=(0,0)(-1,0)(-1,1)(0,-2)(-1,-2)
    // T.ES=(0,0)(1,0)(1,-1)(0,2)(1,2)
    // T.SW=(0,0)(1,0)(1,1)(0,-2)(1,-2)
    // T.WN=(0,0)(-1,0)(-1,-1)(0,2)(-1,2)
    // T.NW=(0,0)(1,0)(1,1)(0,-2)(1,-2)
    // T.WS=(0,0)(-1,0)(-1,-1)(0,2)(-1,2)
    // T.SE=(0,0)(-1,0)(-1,1)(0,-2)(-1,-2)
    // T.EN=(0,0)(1,0)(1,-1)(0,2)(1,2)
    // T.NS=(0,0)(0,1)
    // T.EW=(0,0)(1,0)
    // T.SN=(0,0)(0,-1)
    // T.WE=(0,0)(-1,0)

    kick!(out, N, E, (0, 0)(-1, 0)(-1, 1)(0, -2)(-1, -2));
    kick!(out, E, S, (0, 0)(1, 0)(1, -1)(0, 2)(1, 2));
    kick!(out, S, W, (0, 0)(1, 0)(1, 1)(0, -2)(1, -2));
    kick!(out, W, N, (0, 0)(-1, 0)(-1, -1)(0, 2)(-1, 2));
    kick!(out, N, W, (0, 0)(1, 0)(1, 1)(0, -2)(1, -2));
    kick!(out, W, S, (0, 0)(-1, 0)(-1, -1)(0, 2)(-1, 2));
    kick!(out, S, E, (0, 0)(-1, 0)(-1, 1)(0, -2)(-1, -2));
    kick!(out, E, N, (0, 0)(1, 0)(1, -1)(0, 2)(1, 2));
    kick!(out, N, S, (0, 0)(0, 1));
    kick!(out, E, W, (0, 0)(1, 0));
    kick!(out, S, N, (0, 0)(0, -1));
    kick!(out, W, E, (0, 0)(-1, 0));
    out
};

/// Kick table for I pieces.
pub const KICKS_I: Kicks = {
    let mut out = [[NULL_KICK; Rotation::NB]; Rotation::NB];

    // I.NE=(1,0)(-1,0)(2,0)(-1,-1)(2,2)
    // I.ES=(0,-1)(-1,-1)(2,-1)(-1,1)(2,-2)
    // I.SW=(-1,0)(1,0)(-2,0)(1,1)(-2,-2)
    // I.WN=(0,1)(1,1)(-2,1)(1,-1)(-2,2)
    // I.NW=(0,-1)(-1,-1)(2,-1)(-1,1)(2,-2)
    // I.WS=(1,0)(-1,0)(2,0)(-1,-1)(2,2)
    // I.SE=(0,1)(1,1)(-2,1)(1,-1)(-2,2)
    // I.EN=(-1,0)(1,0)(-2,0)(1,1)(-2,-2)
    // I.NS=(1,-1)(1,0)
    // I.EW=(-1,-1)(0,-1)
    // I.SN=(-1,1)(-1,0)
    // I.WE=(1,1)(0,1)

    kick!(out, N, E, (1, 0)(-1, 0)(2, 0)(-1, -1)(2, 2));
    kick!(out, E, S, (0, -1)(-1, -1)(2, -1)(-1, 1)(2, -2));
    kick!(out, S, W, (-1, 0)(1, 0)(-2, 0)(1, 1)(-2, -2));
    kick!(out, W, N, (0, 1)(1, 1)(-2, 1)(1, -1)(-2, 2));
    kick!(out, N, W, (0, -1)(-1, -1)(2, -1)(-1, 1)(2, -2));
    kick!(out, W, S, (1, 0)(-1, 0)(2, 0)(-1, -1)(2, 2));
    kick!(out, S, E, (0, 1)(1, 1)(-2, 1)(1, -1)(-2, 2));
    kick!(out, E, N, (-1, 0)(1, 0)(-2, 0)(1, 1)(-2, -2));
    kick!(out, N, S, (1, -1)(1, 0));
    kick!(out, E, W, (-1, -1)(0, -1));
    kick!(out, S, N, (-1, 1)(-1, 0));
    kick!(out, W, E, (1, 1)(0, 1));

    out
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{FULL_MASK, LINES, WIDTH};

    /// Direct cell-based placement model, independent of `PMASK`.
    ///
    /// Builds the mask from `CELLS`: the canonical cell at frame
    /// position `(i, j)` occupies board column `cx + i` and board row
    /// `cy + (j - min_y)`, with an explicit per-cell bounds check.
    fn naive_place_mask(piece: Piece, rotation: Rotation, x: i32, y: i32) -> Option<u64> {
        let canon = piece.canonical_rotation(rotation) as usize;
        let (ox, oy) = piece.canonical_offset(rotation);
        let cx = x - ox;
        let cy = y - oy;

        let three = CELLS[piece as usize][canon];
        let cells = [(0i8, 0i8), three[0], three[1], three[2]];

        let mut min_y = 0i32;
        for (_, j) in cells {
            min_y = min_y.min(i32::from(j));
        }

        let mut mask = 0u64;
        for (i, j) in cells {
            let row = cy + i32::from(j) - min_y;
            let col = cx + i32::from(i);
            if !(0..LINES).contains(&row) || !(0..WIDTH).contains(&col) {
                return None;
            }
            mask |= 1u64 << (row * WIDTH + col) as u32;
        }
        Some(mask)
    }

    #[test]
    fn place_mask_matches_cell_model() {
        for piece in Piece::ALL {
            for rotation in Rotation::ALL {
                for x in -6..=16 {
                    for y in -4..=6 {
                        assert_eq!(
                            place_mask(piece, rotation, x, y),
                            naive_place_mask(piece, rotation, x, y),
                            "piece={piece} rotation={rotation:?} x={x} y={y}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn place_mask_never_wraps_or_clips() {
        for piece in Piece::ALL {
            for rotation in Rotation::ALL {
                for x in -6..=16 {
                    for y in -4..=6 {
                        let mask = place_mask(piece, rotation, x, y);
                        let Some(mask) = mask else { continue };
                        assert_eq!(mask.count_ones(), 4, "piece={piece} rotation={rotation:?} x={x} y={y}");
                        assert_eq!(mask | FULL_MASK, FULL_MASK, "wrap outside 40-bit field: piece={piece} rotation={rotation:?} x={x} y={y}");
                    }
                }
            }
        }
    }
}
