use fumen::CellColor;
use fumen::Fumen;
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
            if row[x] == CellColor::Empty {
                continue;
            }
            board.set(x as i32, y as i32);
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

// Rust port of `Marfung37/GluingFumens` unglue method.
//
// A glued fumen holds one piece per page, and its field does not change from
// the first page. Unglue draws every piece onto the field of the first page
// and returns a one-page fumen.
//
// Rows are not removed. Instead, later pieces are moved up past the rows
// that earlier pieces completed. The result is a picture where each piece
// sits where it was played, not a stack.

use fumen::Piece as FumenPiece;
use fumen::PieceType;

use jade_core::data::CELLS;
use jade_core::piece::Piece;
use jade_core::rotation::Rotation;

const WIDTH: i32 = 10;
const HEIGHT: i32 = 23;

/// Unglues a glued fumen.
///
/// Returns the fumen with all pieces drawn onto the field of the first page.
///
/// Errors when the input does not decode or when a piece leaves the field.
pub fn unglue(f: &str) -> Result<String, String> {
    let glued = Fumen::decode(f).map_err(|e| e.to_string())?;

    let page0 = glued
        .pages
        .first()
        .ok_or_else(|| "fumen must have at least one page".to_string())?;

    let mut field = page0.field;

    // Rows modified by the current piece, and rows completed so far.
    let mut rows_modified: u32 = 0;
    let mut rows_cleared: u32 = 0;

    for page in &glued.pages {
        // Pages without a piece place nothing.
        let Some(fumen_piece) = page.piece else {
            continue;
        };

        let (kind, rotation) = piece_of(&fumen_piece);
        let x = fumen_piece.x as i32;
        let y = fumen_piece.y as i32;

        // The four cells of the piece, with the origin cell first.
        let cells = &CELLS[kind as usize][rotation as usize];
        let cells = [(0i8, 0i8), cells[0], cells[1], cells[2]];

        // Every cell must be on the field. The source fails here for any
        // cell outside the field, including above the top.
        if cells.iter().any(|&(cx, cy)| {
            !(0..WIDTH).contains(&(x + i32::from(cx))) || !(0..HEIGHT).contains(&(y + i32::from(cy)))
        }) {
            return Err("one of the pieces goes off the field".to_string());
        }

        let color = color_of(kind);
        for (cx, cy) in cells {
            let row = clear_offset(y + i32::from(cy), rows_cleared);
            field[row as usize][(x + i32::from(cx)) as usize] = color;
            rows_modified |= 1 << row;
        }

        rows_cleared |= find_line_clears(&field, rows_modified);
        rows_modified = 0;
    }

    let mut out = Fumen::default();
    out.add_page().field = field;

    Ok(out.encode())
}

/// Returns the row that a mino ends up at, undoing the line clears that
/// happened below it.
///
/// A cleared row shifts every mino above it down by one. Unglue does not
/// clear the rows, so the shift must be reversed for each row that a
/// previous piece completed below `y`.
const fn clear_offset(y: i32, rows_cleared: u32) -> i32 {
    let mut y = y;
    let mut cleared = rows_cleared;
    let mut mask = (1u32 << (y + 1)) - 1;

    // Clearing a row moves minos above it down. Repeat while the row below
    // the new position was cleared, because the move can uncover another
    // cleared row.
    while cleared & mask != 0 {
        while cleared & mask != 0 {
            cleared &= cleared - 1;
            y += 1;
        }

        mask = (1u32 << (y + 1)) - 1;
    }

    y
}

/// Returns the rows of `rows_modified` that are complete.
fn find_line_clears(field: &[[CellColor; 10]; 23], rows_modified: u32) -> u32 {
    let mut rows_cleared = 0;
    let mut rows_modified = rows_modified;

    while rows_modified != 0 {
        let row = 31 - rows_modified.leading_zeros() as i32;

        if field[row as usize].iter().all(|c| *c != CellColor::Empty) {
            rows_cleared |= 1 << row;
        }

        rows_modified &= !(1 << row);
    }

    rows_cleared
}

/// Converts a fumen piece to a [`Piece`] and a [`Rotation`].
///
/// The `fumen` crate already moved the piece center to the true rotation
/// center on decode, so the rotation needs no further change.
const fn piece_of(piece: &FumenPiece) -> (Piece, Rotation) {
    let kind = match piece.kind {
        PieceType::T => Piece::T,
        PieceType::I => Piece::I,
        PieceType::L => Piece::L,
        PieceType::O => Piece::O,
        PieceType::Z => Piece::Z,
        PieceType::J => Piece::J,
        PieceType::S => Piece::S,
    };

    let rotation = match piece.rotation {
        fumen::RotationState::North => Rotation::North,
        fumen::RotationState::East => Rotation::East,
        fumen::RotationState::South => Rotation::South,
        fumen::RotationState::West => Rotation::West,
    };

    (kind, rotation)
}

/// Returns the field color of a piece.
const fn color_of(kind: Piece) -> CellColor {
    match kind {
        Piece::T => CellColor::T,
        Piece::I => CellColor::I,
        Piece::L => CellColor::L,
        Piece::O => CellColor::O,
        Piece::Z => CellColor::Z,
        Piece::J => CellColor::J,
        Piece::S => CellColor::S,
    }
}
