use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::ops::BitXor;
use std::ops::BitXorAssign;
use std::ops::Not;

use crate::header::LINES;
use crate::header::MASK;
use crate::header::PLAY_LINES;
use crate::header::WIDTH;
use crate::header::col_mask;
use crate::header::dx_mask;
use crate::header::row_word;
use crate::header::rows_below;

/// A single, row-major, 10x6 bitboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board(pub u64);

impl Board {
    /// Returns an empty [`Board`].
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a [`Board`] from its raw 64-bit representation.
    #[inline]
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Creates a [`Board`] with `n` rows filled from the bottom.
    #[inline]
    #[must_use]
    pub const fn lines(n: i32) -> Self {
        let mut w = 0u64;
        let mut row = 0;
        while row < n {
            w |= row_word(row);
            row += 1;
        }
        Self(w)
    }

    /// Returns the total number of filled cells in the board.
    #[inline]
    #[must_use]
    pub const fn popcount(self) -> u32 {
        self.0.count_ones()
    }

    /// Shifts every lane by `(dx, dy)`.
    ///
    /// `dy > 0` shifts up; `dy < 0` shifts down. `dx > 0` shifts right;
    /// `dx < 0` shifts left. Bits shifted past an edge are dropped.
    #[inline]
    #[must_use]
    pub const fn shifted(self, dx: i32, dy: i32) -> Self {
        let dy_shift = dy * 10;

        let v = if dy_shift == 0 {
            self.0
        } else if dy_shift > 0 {
            self.0 << dy_shift
        } else {
            self.0 >> (-dy_shift)
        };

        let out = if dx == 0 {
            v
        } else if dx > 0 {
            v << dx as u64
        } else {
            v >> (-dx) as u64
        } & dx_mask(dx);

        Self(out)
    }

    /// Directly shifts the board left by 1 cell.
    #[inline]
    #[must_use]
    pub const fn shl(self) -> Self {
        Self((self.0 << 1) & dx_mask(1))
    }

    /// Directly shifts the board right by 1 cell.
    #[inline]
    #[must_use]
    pub const fn shr(self) -> Self {
        Self((self.0 >> 1) & dx_mask(-1))
    }

    /// Directly shifts the board down by 1 row.
    #[inline]
    #[must_use]
    pub const fn shd(self) -> Self {
        Self(self.0 >> 10)
    }

    /// Returns whether the cell at `(x, y)` is set.
    #[inline]
    #[must_use]
    pub const fn get(self, x: i32, y: i32) -> bool {
        // debug_assert!(
        //     (0..WIDTH).contains(&x) && (0..LINES).contains(&y),
        //     "out-of-bounds access to Board::get({x}, {y})"
        // );
        let bit = 1u64 << (y * WIDTH + x) as u32;
        self.0 & bit != 0
    }

    /// Returns whether any cell is set on the board.
    #[inline]
    #[must_use]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    /// Sets the cell `(x, y)` in lane `lane`.
    #[inline]
    pub const fn set(&mut self, x: i32, y: i32) {
        self.0 |= 1u64 << (y * 10 + x) as u32;
    }

    /// Returns the occupied height of lane `lane`, or `0` if it is empty.
    #[inline]
    #[must_use]
    pub const fn height(self) -> i32 {
        if self.0 != 0 {
            let idx = 64 - 1 - self.0.leading_zeros() as i32;
            return idx / WIDTH + 1;
        }

        0
    }

    /// Returns whether the play field has an empty cell that cannot be
    /// filled.
    ///
    /// An empty cell is bounded when both neighbors in its row are filled,
    /// or when one side is a wall and the other is filled.  If a column is
    /// not full but every empty cell in it is bounded, then the column can
    /// only be filled by one vertical I piece.  With any filled cell already
    /// in the column, the column cannot be filled.  The board is then
    /// impossible to fill.
    ///
    /// Only the bottom `PLAY_LINES` rows are checked.  The rows above are
    /// a piece movement margin and never hold filled cells.
    ///
    /// Each row is shifted down so that its column bits sit in bits 0-9,
    /// forming 10-bit column vectors.  Shifts that wrap cell bits across row
    /// boundaries are neutralized by the wall masks.  Bits above bit 9 are
    /// garbage and are cleared by `bounded`.
    #[inline]
    #[must_use]
    pub const fn has_isolated_cell(self) -> bool {
        const FIELD: u64 = rows_below(PLAY_LINES);
        const LEFT_WALL: u64 = col_mask(0) & FIELD;
        const RIGHT_WALL: u64 = col_mask(9) & FIELD;

        let b = self.0 & FIELD;

        let full = (b >> 30) & (b >> 20) & (b >> 10) & b;
        let not_empty = (b >> 30) | (b >> 20) | (b >> 10) | b;

        let left_bounded = (b << 1) | LEFT_WALL;
        let right_bounded = (b >> 1) | RIGHT_WALL;

        let bounded_cells = (left_bounded & right_bounded) | b;

        let bounded =
            (bounded_cells >> 30) & (bounded_cells >> 20) & (bounded_cells >> 10) & bounded_cells;

        // A column that is neither empty nor full, and whose every empty cell
        // is bounded, makes the board impossible.  `bounded` has no bits set
        // above bit 9, so the garbage in `not_empty` and `!full` is cleared.
        (not_empty & !full & bounded) != 0
    }

    /// Returns whether the play field has a disconnected region whose empty
    /// cell count is not a multiple of four.
    ///
    /// Each placed piece fills four cells.  A region separated from the rest
    /// of the board can only be filled by pieces placed entirely inside it.
    /// A region whose empty cell count is not a multiple of four can never be
    /// filled, so the board is impossible.
    ///
    /// When two adjacent columns have a filled cell in every row, no empty
    /// cell can cross the seam between them.  The cells left of the seam are
    /// then disconnected from the cells right of it.
    ///
    /// Only the bottom `PLAY_LINES` rows are checked.  The rows above are
    /// a piece movement margin and never hold filled cells.
    ///
    /// Columns 1 to 7 are checked.  Checking columns 0 and 8 as well is
    /// equivalent to checking `has_isolated_cell`, which is cheaper.
    #[inline]
    #[must_use]
    pub const fn has_imbalanced_split(self) -> bool {
        // Column 0 across the play-field rows.
        const COL_0: u64 = col_mask(0) & rows_below(PLAY_LINES);

        let b = self.0 & rows_below(PLAY_LINES);

        let mut col_bits = COL_0;
        let mut left_bits = COL_0;

        let mut col = 1;
        while col <= 7 {
            col_bits <<= 1;
            left_bits |= col_bits;

            // Every row of the seam between `col` and `col + 1` cells has a
            // filled cell, so cells left of the seam cannot reach cells right
            // of it.
            if ((b | (b >> 1)) & col_bits) == col_bits {
                // A left region has 4*cols cells, so its empty count is a
                // multiple of four exactly when the filled count is.
                let left = b & left_bits;
                if !left.count_ones().is_multiple_of(4) {
                    return true;
                }
            }

            col += 1;
        }

        false
    }

    /// Moves all completely filled rows to the bottom of the board, shifting
    /// the residue up.
    #[inline]
    #[must_use]
    pub const fn clearshift(self) -> Self {
        const ROW_MASK: u64 = 0x3FF;
        const WIDTH_SHIFT: u32 = 10;

        let b = self.0;

        let r0 = b & ROW_MASK;
        let r1 = (b >> WIDTH_SHIFT) & ROW_MASK;
        let r2 = (b >> (2 * WIDTH_SHIFT)) & ROW_MASK;
        let r3 = (b >> (3 * WIDTH_SHIFT)) & ROW_MASK;
        let r4 = (b >> (4 * WIDTH_SHIFT)) & ROW_MASK;
        let r5 = (b >> (5 * WIDTH_SHIFT)) & ROW_MASK;

        let f0 = (r0 == ROW_MASK) as u64;
        let f1 = (r1 == ROW_MASK) as u64;
        let f2 = (r2 == ROW_MASK) as u64;
        let f3 = (r3 == ROW_MASK) as u64;
        let f4 = (r4 == ROW_MASK) as u64;
        let f5 = (r5 == ROW_MASK) as u64;

        let total_filled = f0 + f1 + f2 + f3 + f4 + f5;

        let nf0 = 1 - f0;
        let nf1 = 1 - f1;
        let nf2 = 1 - f2;
        let nf3 = 1 - f3;
        let nf4 = 1 - f4;
        let nf5 = 1 - f5;

        let p0_f = 0u32;
        let p1_f = f0 as u32;
        let p2_f = (f0 + f1) as u32;
        let p3_f = (f0 + f1 + f2) as u32;
        let p4_f = (f0 + f1 + f2 + f3) as u32;
        let p5_f = (f0 + f1 + f2 + f3 + f4) as u32;

        let p0_e = total_filled as u32;
        let p1_e = (total_filled + nf0) as u32;
        let p2_e = (total_filled + nf0 + nf1) as u32;
        let p3_e = (total_filled + nf0 + nf1 + nf2) as u32;
        let p4_e = (total_filled + nf0 + nf1 + nf2 + nf3) as u32;
        let p5_e = (total_filled + nf0 + nf1 + nf2 + nf3 + nf4) as u32;

        let mut out = 0u64;
        out |= (ROW_MASK * f0) << (p0_f * WIDTH_SHIFT);
        out |= (ROW_MASK * f1) << (p1_f * WIDTH_SHIFT);
        out |= (ROW_MASK * f2) << (p2_f * WIDTH_SHIFT);
        out |= (ROW_MASK * f3) << (p3_f * WIDTH_SHIFT);
        out |= (ROW_MASK * f4) << (p4_f * WIDTH_SHIFT);
        out |= (ROW_MASK * f5) << (p5_f * WIDTH_SHIFT);

        out |= (r0 * nf0) << (p0_e * WIDTH_SHIFT);
        out |= (r1 * nf1) << (p1_e * WIDTH_SHIFT);
        out |= (r2 * nf2) << (p2_e * WIDTH_SHIFT);
        out |= (r3 * nf3) << (p3_e * WIDTH_SHIFT);
        out |= (r4 * nf4) << (p4_e * WIDTH_SHIFT);
        out |= (r5 * nf5) << (p5_e * WIDTH_SHIFT);

        Self(out)
    }

    /// Returns whether any of the six rows is completely filled.
    #[inline]
    #[must_use]
    pub const fn has_full_row(self) -> bool {
        const ROW: u64 = 0x3FF;
        let b = self.0;
        (b & ROW) == ROW
            || ((b >> 10) & ROW) == ROW
            || ((b >> 20) & ROW) == ROW
            || ((b >> 30) & ROW) == ROW
            || ((b >> 40) & ROW) == ROW
            || ((b >> 50) & ROW) == ROW
    }

    /// Returns an iterator over the positions of filled cells in the board.
    #[inline]
    #[must_use]
    pub const fn iter(&self) -> BoardIter<'_> {
        BoardIter {
            board: self,
            x: 0,
            y: 0,
        }
    }
}

impl<'a> IntoIterator for &'a Board {
    type Item = (i32, i32);
    type IntoIter = BoardIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over filled positions within a [`Board`].
pub struct BoardIter<'a> {
    board: &'a Board,
    x: i32,
    y: i32,
}

impl Iterator for BoardIter<'_> {
    type Item = (i32, i32);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.y < LINES {
            while self.x < WIDTH {
                if self.board.get(self.x, self.y) {
                    let pos = (self.x, self.y);
                    self.x += 1;
                    return Some(pos);
                }

                self.x += 1;
            }

            self.x = 0;
            self.y += 1;
        }

        None
    }
}

impl BitAnd for Board {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for Board {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitXor for Board {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitAndAssign for Board {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for Board {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for Board {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for Board {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0 & MASK)
    }
}

impl From<u64> for Board {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
