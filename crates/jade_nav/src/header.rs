use jade_core::data::CELLS;
use jade_core::piece::Piece;
use jade_core::rotation::Rotation;

pub use jade_core::header::WIDTH;

pub const LINES: i32 = 6;
pub const BITS: u32 = 60;
pub const MASK: u64 = (1u64 << BITS) - 1;

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

#[cfg(test)]
mod tests {
    use super::*;
    use jade_core::data::place_mask;

    #[test]
    fn landed_ok_rows_follow_frame_span() {
        for piece in Piece::ALL {
            for rotation in Rotation::ALL {
                let canon = piece.canonical_rotation(rotation) as usize;
                let three = CELLS[piece as usize][canon];
                let cells = [(0i8, 0i8), three[0], three[1], three[2]];

                let mut min_y = 0i32;
                let mut max_y = 0i32;
                for (_, j) in cells {
                    min_y = min_y.min(i32::from(j));
                    max_y = max_y.max(i32::from(j));
                }
                let top = 3 - (max_y - min_y);

                for row in 0..LINES {
                    let allowed = (LANDED_OK[piece as usize][canon] & row_word(row)) != 0;
                    assert_eq!(allowed, row <= top, "piece={piece} rotation={rotation:?} row={row}");
                }
            }
        }
    }

    #[test]
    fn landed_ok_is_necessary_for_place_mask() {
        for piece in Piece::ALL {
            for rotation in Rotation::ALL {
                let canon = piece.canonical_rotation(rotation) as usize;
                let (ox, oy) = piece.canonical_offset(rotation);
                for x in 0..10 {
                    for y in 0..8 {
                        if place_mask(piece, rotation, x, y).is_some() {
                            let cx = x - ox;
                            let cy = y - oy;
                            let bit = 1u64 << (cy * WIDTH + cx) as u32;
                            assert_ne!(
                                LANDED_OK[piece as usize][canon] & bit,
                                0,
                                "piece={piece} rotation={rotation:?} x={x} y={y}",
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn known_landed_ok_rows() {
        let i_vertical = Piece::I.canonical_rotation(Rotation::East) as usize;
        assert_eq!(LANDED_OK[Piece::I as usize][i_vertical], row_word(0));

        let t_north = Piece::T.canonical_rotation(Rotation::North) as usize;
        assert_eq!(
            LANDED_OK[Piece::T as usize][t_north],
            row_word(0) | row_word(1) | row_word(2),
        );
    }
}