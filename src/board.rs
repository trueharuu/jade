use crate::header::{BOARD_MASK, HEIGHT, ROW0, ROW9, WIDTH, row_span_mask};

/// A 10x4 row-major bitboard, with the highest 24 bits unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board(u64);

impl Board {
    /// Creates a new, empty board.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Board(0)
    }

    /// Retrieves the bit at `(x, y)`.
    #[inline]
    #[must_use]
    pub const fn get(&self, x: u32, y: u32) -> bool {
        assert!(x < WIDTH && y < HEIGHT);
        (self.0 & (1 << (y * WIDTH + x))) != 0
    }

    /// Sets the bit at `(x, y)` to 1.
    #[inline]
    pub const fn set(&mut self, x: u32, y: u32) {
        assert!(x < WIDTH && y < HEIGHT);
        self.0 |= 1 << (y * WIDTH + x);
    }

    /// Clears the bit at `(x, y)` to 0.
    #[inline]
    pub const fn clear(&mut self, x: u32, y: u32) {
        assert!(x < WIDTH && y < HEIGHT);
        self.0 &= !(1 << (y * WIDTH + x));
    }

    #[inline]
    #[must_use]
    pub const fn shifted(&self, dx: i32, dy: i32) -> Self {
        let mut result = if dy >= 0 {
            self.0 << (dy as u32 * WIDTH)
        } else {
            self.0 >> (-dy as u32 * WIDTH)
        };

        if dx != 0 {
            result = if dx > 0 {
                result << (dx as usize)
            } else {
                result >> (-dx as usize)
            };

            result &= row_span_mask(dx);
        }

        Self(result & BOARD_MASK)
    }

    /// Returns a column mask selecting full lines.
    #[inline]
    #[must_use]
    pub const fn line_clears(&self) -> Self {
        let d = self.0;
        let summed = (d & !ROW9) + ROW0;
        Self(d & summed & ROW9)
    }

    /// Moves all lines flagged in `lines`, compacting them downwards.
    pub const fn clearshift(&mut self, lines: Self) {
        let mut d = self.0;
        let mut clears = lines.0;

        while clears != 0 {
            let y = clears.trailing_zeros();

            let below = (1u64 << y) - 1;
            d = (d & below) | ((d >> HEIGHT) & !below);

            clears >>= HEIGHT;
        }

        self.0 = d;
    }
}
