use fumen::{CellColor, Fumen};
use jade_core::board::Board;

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