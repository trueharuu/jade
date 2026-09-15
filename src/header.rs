pub const WIDTH: u32 = 10;
/// Working rows of the field.
pub const HEIGHT: u32 = 4;
/// Rows per band of the banded bitboard. A placement never crosses a band.
pub const TLINES: u32 = 6;
/// Bits in one band element.
pub const BOARD_BITS: u32 = WIDTH * TLINES;
pub const ROW_MASK: u64 = (1 << WIDTH) - 1;
pub const BOARD_MASK: u64 = (1 << BOARD_BITS) - 1;

#[inline]
#[must_use]
pub const fn row_span_mask(dx: i32) -> u64 {
    let row_bits = if dx < 0 {
        ROW_MASK >> (-dx as usize)
    } else {
        ROW_MASK << (dx as usize)
    } & ROW_MASK;

    let mut m = 0;
    let mut r = 0;
    while r < HEIGHT {
        m |= row_bits << (r * WIDTH);
        r += 1;
    }

    m
}

pub const ROW0: u64 = {
    // bit 0 of every row
    let mut m = 0u64;
    let mut r = 0u32;
    while r < HEIGHT { m |= 1u64 << (r * WIDTH); r += 1; }
    m
};

pub const ROW9: u64 = {
    // bit (WIDTH-1) of every row
    let mut m = 0u64;
    let mut r = 0u32;
    while r < HEIGHT { m |= 1u64 << (r * WIDTH + WIDTH - 1); r += 1; }
    m
};

