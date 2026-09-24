use fumen::{CellColor, Fumen};
use jade_core::board::Board;

pub mod model;
pub mod perft;

#[inline]
#[must_use]
pub fn human(value: f64) -> String {
    if value < 1_000.0 {
        format!("{value:.2}")
    } else if value < 1_000_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else if value < 1_000_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value < 1_000_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    }
}

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

    Ok(board)
}
