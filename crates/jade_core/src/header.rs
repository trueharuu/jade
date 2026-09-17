pub const WIDTH: i32 = 10;
pub const LINES: i32 = 4;
pub const TOTAL_BITS: u32 = (WIDTH * LINES) as u32;
pub const FULL_MASK: u64 = (1u64 << TOTAL_BITS) - 1;

#[inline]
#[must_use]
pub const fn col_word(x: i32) -> u64 {
    let mut w = 0u64;
    let mut row = 0;
    while row < LINES {
        w |= 1u64 << (row * WIDTH + x);
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
        w |= col_word(c);
        c += 1;
    }

    w
}

/// Mask that keeps only the bits that stay on-board after a
/// horizontal shift by `dx` columns. Without this, a shift would wrap
/// a piece around the left or right wall into the opposite edge.
#[inline]
#[must_use]
pub const fn dx_mask(dx: i32) -> u64 {
    if dx > 0 {
        FULL_MASK & !cols_below(dx)
    } else if dx < 0 {
        FULL_MASK & !(cols_below(-dx) << (WIDTH + dx) as u32)
    } else {
        FULL_MASK
    }
}
