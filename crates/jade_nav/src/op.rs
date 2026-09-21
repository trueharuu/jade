use jade_core::board::Board;
use jade_core::data::CELLS;
use jade_core::data::KICKS_I;
use jade_core::data::KICKS_TJLSZ;
use jade_core::header::LINES;
use jade_core::header::PLAY_LINES;
use jade_core::header::WIDTH;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rot_idx;
use jade_core::rotation::Rotation;

#[inline]
#[must_use]
pub const fn check<const P: Piece>(
    usable: &[Board; Rotation::NB],
    x: i32,
    y: i32,
    r: usize,
) -> bool {
    // canonicalize the position and rotation
    let (dx, dy) = P.canonical_offset(rot_idx!(r));
    let r = P.canonical_rotation(rot_idx!(r)) as usize;
    let ox = x - dx;
    let oy = y - dy;
    if ox < 0 || ox >= WIDTH || oy < 0 || oy >= LINES {
        return false;
    }

    usable[r].get(ox, oy)
}

/// Returns origin positions where the `I`-th mino of `(P, R)` can be placed
/// safely.
#[inline]
#[must_use]
pub fn usable_cell<const P: Piece, const R: Rotation, const I: usize>(
    b: &Board,
    nb: &Board,
) -> Board {
    let cx = i32::from(CELLS[P as usize][R as usize][I].0);
    let cy = i32::from(CELLS[P as usize][R as usize][I].1);

    let usable = if cy > 0 {
        (!b.shifted(0, -cy)).shifted(-cx, 0)
    } else {
        nb.shifted(-cx, -cy)
    };

    // The mino cell (cx, cy) must itself be inside the 10x6 board.
    // Shift the board mask into origin-space to obtain the valid origins.
    usable & Board::lines(6).shifted(-cx, -cy)
}

/// Returns origin positions where all four minos of `x(P, R)` are
/// collision-free.
#[inline]
#[must_use]
pub fn usable_rot<const P: Piece, const R: Rotation>(b: &Board, nb: &Board) -> Board {
    *nb & usable_cell::<P, R, 0>(b, nb)
        & usable_cell::<P, R, 1>(b, nb)
        & usable_cell::<P, R, 2>(b, nb)
}

/// Builds usable-position maps for every canonical rotation of `P`.
#[inline]
#[must_use]
pub fn usable_map<const P: Piece>(b: &Board) -> [Board; Rotation::NB] {
    let negated = !*b;
    let mut usable = [Board::empty(); Rotation::NB];
    usable[0] = usable_rot::<P, { Rotation::North }>(b, &negated);

    if P.group2() || P.group4() {
        usable[1] = usable_rot::<P, { Rotation::East }>(b, &negated);
    }

    if P.group4() {
        usable[2] = usable_rot::<P, { Rotation::South }>(b, &negated);
        usable[3] = usable_rot::<P, { Rotation::West }>(b, &negated);
    }

    usable
}

/// Origins of `(P, R)` whose four cells all land in the play field.
///
/// The play field is the bottom `PLAY_LINES` rows; intermediate search
/// positions may sit higher.
#[inline]
#[must_use]
pub(crate) fn fit_rot<const P: Piece, const R: Rotation>() -> Board {
    let region = Board::lines(PLAY_LINES);
    let mut fit = region;
    let mut i = 0;
    while i < 3 {
        let cx = i32::from(CELLS[P as usize][R as usize][i].0);
        let cy = i32::from(CELLS[P as usize][R as usize][i].1);
        fit &= region.shifted(-cx, -cy);
        i += 1;
    }
    fit
}

/// Builds the per-rotation origins where every cell of `P` lies in the
/// play field, for the canonical rotations of `P`.
#[inline]
#[must_use]
pub(crate) fn fit_map<const P: Piece>() -> [Board; Rotation::NB] {
    let mut fit = [Board::empty(); Rotation::NB];
    fit[0] = fit_rot::<P, { Rotation::North }>();

    if P.group2() || P.group4() {
        fit[1] = fit_rot::<P, { Rotation::East }>();
    }

    if P.group4() {
        fit[2] = fit_rot::<P, { Rotation::South }>();
        fit[3] = fit_rot::<P, { Rotation::West }>();
    }

    fit
}

/// Converts usable maps into landable maps by requiring support directly below.
#[inline]
#[must_use]
pub fn landable_map(u: &[Board; Rotation::NB], cs: usize) -> [Board; Rotation::NB] {
    let mut c = [Board::empty(); Rotation::NB];
    for r in 0..cs {
        c[r] = u[r] & !u[r].shifted(0, 1);
    }
    c
}

/// Attempts to rotate `mv` to `target`, applying the first valid kick; returns
/// the original move when no kick succeeds.
#[inline]
#[must_use]
pub fn apply_rotation<const P: Piece>(
    usable: &[Board; Rotation::NB],
    mv: &Move,
    target: Rotation,
) -> Move {
    if mv.rotation() == target || matches!(P, Piece::O) {
        return *mv;
    }

    let kt = match mv.piece() {
        Piece::I => KICKS_I,
        Piece::O => unsafe { std::hint::unreachable_unchecked() },
        _ => KICKS_TJLSZ,
    };

    let lane = kt[mv.rotation() as usize][target as usize];
    for kick_idx in 0..lane.1 {
        let dx = i32::from(lane.0[kick_idx].0);
        let dy = i32::from(lane.0[kick_idx].1);
        let new_x = mv.x() + dx;
        let new_y = mv.y() + dy;
        if check::<P>(usable, new_x, new_y, target as usize) {
            return Move::new(mv.piece(), new_x, new_y, target);
        }
    }

    *mv
}
