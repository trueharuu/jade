use crate::data::CELLS;
use crate::piece::Piece;
use crate::rotation::Rotation;

pub const WIDTH: i32 = 10;
pub const LINES: i32 = 6;
pub const BITS: u32 = 60;
pub const MASK: u64 = (1u64 << BITS) - 1;

/// Spawn column and row of the piece origin.
pub const SPAWN_X: i32 = 4;
pub const SPAWN_Y: i32 = 4;

/// One full row of `WIDTH` bits at `row`.
#[inline]
#[must_use]
pub const fn row_word(row: i32) -> u64 {
    ((1u64 << WIDTH as u32) - 1) << (row * WIDTH) as u32
}

/// All bits in column `x` across all `LINES` rows.
#[inline]
#[must_use]
pub const fn col_mask(x: i32) -> u64 {
    let mut w = 0u64;
    let mut row = 0;
    while row < LINES {
        w |= 1u64 << (row * WIDTH + x) as u32;
        row += 1;
    }
    w
}

/// Mask covering columns `[0, n)`.
#[inline]
#[must_use]
pub const fn cols_below(n: i32) -> u64 {
    let mut w = 0u64;
    let mut c = 0;
    while c < n {
        w |= col_mask(c);
        c += 1;
    }
    w
}

/// Mask covering rows `[0, n)`.
#[inline]
#[must_use]
pub const fn rows_below(n: i32) -> u64 {
    let mut w = 0u64;
    let mut row = 0;
    while row < n {
        w |= row_word(row);
        row += 1;
    }
    w
}

/// Mask that keeps only the bits that stay on-field after a horizontal
/// shift by `dx` columns.
#[inline]
#[must_use]
pub const fn dx_mask(dx: i32) -> u64 {
    if dx > 0 {
        MASK & !cols_below(dx)
    } else if dx < 0 {
        MASK & !(cols_below(-dx) << (WIDTH + dx) as u32)
    } else {
        MASK
    }
}

/// Lowest row of the `(p, r)` frame relative to its origin.
///
/// The placement model in `jade_core` normalizes every canonical frame
/// so that the lowest cell in the frame sits at the frame's bottom row.
/// An origin at row `cy` therefore places its cells at rows
/// `cy + (j - min_y)`.
#[inline]
#[must_use]
pub const fn frame_min_y(p: usize, r: usize) -> i32 {
    let three = CELLS[p][r];
    let mut mn = 0i32;
    let mut i = 0;
    while i < 3 {
        let y = three[i].1 as i32;
        if y < mn {
            mn = y;
        }
        i += 1;
    }
    mn
}

/// Allowed locked-origin rows per `(piece, rotation)`, in the 60-bit
/// plane. A locked origin at row `cy` and column `cx` is valid only
/// when every placed cell sits in rows 0 to 3.
///
/// The canonical frame of `(piece, canonic)` spans rows `min_y` to
/// `max_y` above its origin. Cells occupy rows `cy + (j - min_y)`, so
/// the lock rule is `cy + (max_y - min_y) <= 3`. The columns are
/// bounded by the placement plane itself and are not part of this mask.
pub const LANDED_OK: [[u64; Rotation::NB]; Piece::NB] = const {
    let mut out = [[0u64; Rotation::NB]; Piece::NB];

    let mut p = 0;
    while p < Piece::NB {
        let mut rc = 0;
        while rc < Rotation::NB {
            let three = CELLS[p][rc];
            let cells = [(0i8, 0i8), three[0], three[1], three[2]];

            let mut min_y = 0i32;
            let mut max_y = 0i32;
            let mut i = 0;
            while i < 4 {
                let y = cells[i].1 as i32;
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
                i += 1;
            }

            let top = 3 - (max_y - min_y);
            let mut row = 0;
            while row <= top {
                out[p][rc] |= row_word(row);
                row += 1;
            }

            rc += 1;
        }
        p += 1;
    }
    out
};
