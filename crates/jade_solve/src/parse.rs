use std::fmt::Display;
use std::str::FromStr;

use fumen::CellColor;
use fumen::Fumen;
use jade_core::board::Board;
use jade_core::piece::Piece;

/// Decodes a one-page fumen into a `Board` of the first four rows.
///
/// Only grey (filled) cells are kept. The page must have exactly one page.
pub fn parse_fumen(s: &str) -> Result<Board, String> {
    let f = Fumen::decode(s).map_err(|x| x.to_string())?;
    if f.pages.len() != 1 {
        return Err(format!(
            "fumen must have exactly one page, got {}",
            f.pages.len()
        ));
    }

    let page = f.pages[0].field;
    let mut board = Board::empty();

    for (y, row) in page.iter().enumerate().take(4) {
        for (x, _) in row.iter().enumerate().take(10) {
            if row[x] == CellColor::Grey {
                board.set(x as i32, y as i32);
            }
        }
    }

    Ok(board.clearshift())
}

/// Encodes a [`Board`] into a one-page fumen.
/// Only the first four rows are encoded, and only grey (filled) cells are kept.
#[inline]
#[must_use]
pub fn encode_fumen(board: &Board) -> String {
    let mut f = Fumen::default();
    let p = f.add_page();
    for y in 0..4 {
        for x in 0..10 {
            if board.get(x, y) {
                p.field[y as usize][x as usize] = CellColor::Grey;
            }
        }
    }

    f.encode()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saves(u8);

impl Saves {
    pub const fn empty() -> Self {
        Saves(0)
    }

    pub const fn size(self) -> usize {
        self.0.count_ones() as usize
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    // piece is from 0..=7
    #[must_use]
    pub const fn has(&self, piece: Piece) -> bool {
        self.0 & (1 << (piece as u8)) != 0
    }
}

impl FromStr for Saves {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut saves = 0;
        for c in s.chars() {
            let piece =
                Piece::from_str(&c.to_string()).map_err(|_| format!("invalid piece: {c}"))?;
            saves |= 1 << (piece as u8);
        }
        Ok(Saves(saves))
    }
}

impl Display for Saves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for piece in [
            Piece::T,
            Piece::I,
            Piece::J,
            Piece::L,
            Piece::O,
            Piece::S,
            Piece::Z,
        ] {
            if self.has(piece) {
                write!(f, "{piece}")?;
            }
        }

        Ok(())
    }
}
