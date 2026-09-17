use jade_core::board::Plane;
use jade_core::data::CELLS;
use jade_core::data::KICKS_I;
use jade_core::data::KICKS_TJLSZ;
use jade_core::header;
use jade_core::header::LINES;
use jade_core::header::WIDTH;
use jade_core::piece::Piece;
use jade_core::placement::Move;
use jade_core::rotation::Rotation;

#[macro_export]
/// Runs `body` for the compile-time rotation literals 0 to 3, each
/// guarded by the run-time limit `$limit`.
macro_rules! unroll {
    ($r:ident, $limit:expr, $body:block) => {{
        {
            #[allow(non_upper_case_globals)]
            const $r: usize = 0;
            if $r < $limit $body
        }
        {
            #[allow(non_upper_case_globals)]
            const $r: usize = 1;
            if $r < $limit $body
        }
        {
            #[allow(non_upper_case_globals)]
            const $r: usize = 2;
            if $r < $limit $body
        }
        {
            #[allow(non_upper_case_globals)]
            const $r: usize = 3;
            if $r < $limit $body
        }
    }};
}

/// Origin positions where the `I`-th mino of `(P, R)` can be placed.
///
/// Minos use the same normalized frame as `jade_core::data::place_mask`.
/// A mino above the origin (positive `cy`) changes the usable result
/// through the negated board. The result is restricted to origin rows
/// that keep the mino inside the plane, since the plane top is the
/// working-field top and above-top cells are not legal intermediate
/// states.
#[inline]
#[must_use]
pub fn usable_cell<const N: usize, const P: Piece, const R: usize, const I: usize>(
    b: &Plane<N>,
    nb: &Plane<N>,
) -> Plane<N> {
    // `jade_core` places locks through normalized frames: a cell at raw
    // offset `(x, y)` occupies board row `origin_row + (y - min_y)`.
    // Reachability must use the same normalized offsets, or the locked
    // mask will not match the piece that fell.
    let my = header::frame_min_y(P as usize, R);
    let cx = i32::from(CELLS[P as usize][R][I].0);
    let cy = i32::from(CELLS[P as usize][R][I].1) - my;
    usable_component(b, nb, cx, cy)
}

#[inline]
#[must_use]
fn usable_component<const N: usize>(b: &Plane<N>, nb: &Plane<N>, cx: i32, cy: i32) -> Plane<N> {
    match cy {
        1.. => {
            let free = !b.shifted(0, -cy);
            let in_plane = Plane::splat(header::rows_below(LINES - cy));
            (free & in_plane).shifted(-cx, 0)
        }
        ..0 => nb.shifted(-cx, -cy),
        0 => nb.shifted(-cx, 0),
    }
}

/// Origin positions where all four minos of `(P, R)` are collision-free
/// and inside the plane.
#[inline]
#[must_use]
pub fn usable_rot<const N: usize, const P: Piece, const R: usize>(
    b: &Plane<N>,
    nb: &Plane<N>,
) -> Plane<N> {
    let my = header::frame_min_y(P as usize, R);
    // The origin cell also normalizes: it occupies row `origin_row - min_y`.
    usable_component(b, nb, 0, -my)
        & usable_cell::<N, P, R, 0>(b, nb)
        & usable_cell::<N, P, R, 1>(b, nb)
        & usable_cell::<N, P, R, 2>(b, nb)
}

/// Builds usable-position maps for every canonical rotation of `P`.
#[inline]
#[must_use]
pub fn usable_map<const N: usize, const P: Piece>(b: Plane<N>) -> [Plane<N>; Rotation::NB] {
    let nb = !b;
    let mut usable = [Plane::empty(); Rotation::NB];
    usable[0] = usable_rot::<N, P, 0>(&b, &nb);

    if P.group2() || P.group4() {
        usable[1] = usable_rot::<N, P, 1>(&b, &nb);
    }

    if P.group4() {
        usable[2] = usable_rot::<N, P, 2>(&b, &nb);
        usable[3] = usable_rot::<N, P, 3>(&b, &nb);
    }

    usable
}

/// Converts usable maps into landable maps by requiring support below.
#[inline]
#[must_use]
pub fn landable_map<const N: usize>(
    u: &[Plane<N>; Rotation::NB],
    cs: usize,
) -> [Plane<N>; Rotation::NB] {
    let mut c = [Plane::empty(); Rotation::NB];
    for r in 0..cs {
        c[r] = u[r] & !u[r].shifted(0, 1);
    }
    c
}

/// Origins reached by one soft-drop input from any origin in `s`.
///
/// The piece falls to the bottom of its usable column run in one input.
#[inline]
#[must_use]
pub fn vertical_drop<const N: usize>(s: Plane<N>, usable: &Plane<N>) -> Plane<N> {
    let resting = *usable & !usable.shifted(0, 1);
    sonic_drop(s, usable) & resting
}

/// Downward closure of `s` within `usable`: every cell the frontier
/// falls through.
#[inline]
#[must_use]
pub fn sonic_drop<const N: usize>(mut s: Plane<N>, usable: &Plane<N>) -> Plane<N> {
    loop {
        let new = s.shifted(0, -1) & *usable & !s;
        if !new.any() {
            break;
        }
        s |= new;
    }
    s
}

/// Expands a blocking surface downward by powers of two.
///
/// The ceiling is at most `LINES` minus the generation height, so the
/// downward expansion never exceeds the plane.
#[inline]
#[must_use]
pub fn vertical_ceiling<const N: usize>(mut surface: Plane<N>, ceiling: i32) -> Plane<N> {
    if ceiling >= 1 {
        surface |= surface.shifted(0, -1);
    }

    if ceiling >= 2 {
        surface |= surface.shifted(0, -2);
    }

    if ceiling >= 4 {
        surface |= surface.shifted(0, -4);
    }

    surface
}

/// Expands a frontier by one horizontal step left and right within
/// usable cells.
#[inline]
#[must_use]
pub fn horizontal_tuck<const N: usize>(s: Plane<N>, usable: &Plane<N>) -> Plane<N> {
    (s.shifted(1, 0) | s.shifted(-1, 0)) & *usable
}

/// Computes the min/max kick reach `(xmin, xmax, ymin, ymax)` across
/// cw and ccw kicks.
#[must_use]
pub const fn env_union(p: Piece, r: usize) -> (i32, i32, i32, i32) {
    let kt = match p {
        Piece::I => &KICKS_I,
        _ => &KICKS_TJLSZ,
    };
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    let mut d = 0;
    while d < 2 {
        let r1 = if d == 0 { (r + 1) & 3 } else { (r + 3) & 3 };
        let mut i = 0;
        while i < 5 {
            let kx = kt[r][r1].0[i].0 as i32;
            let ky = kt[r][r1].0[i].1 as i32;
            if kx < xmin {
                xmin = kx;
            }
            if kx > xmax {
                xmax = kx;
            }
            if ky < ymin {
                ymin = ky;
            }
            if ky > ymax {
                ymax = ky;
            }
            i += 1;
        }
        d += 1;
    }
    (xmin, xmax, ymin, ymax)
}

/// Compile-time envelope accessor specialized by piece and rotation.
pub struct EnvelopeTable<const P: Piece, const R: usize>;

impl<const P: Piece, const R: usize> EnvelopeTable<P, R> {
    /// Envelope bounds for `(P, R)`.
    pub const E: (i32, i32, i32, i32) = env_union(P, R);
}

/// Expands occupied cells by envelope reach to produce candidate
/// collision probes.
#[inline]
#[must_use]
pub fn env_probe<const N: usize>(s: &Plane<N>, e: (i32, i32, i32, i32)) -> Plane<N> {
    let (xmin, xmax, ymin, ymax) = e;
    let mut h = *s;
    if xmin <= -1 {
        h |= s.shifted(-1, 0);
    }
    if xmin <= -2 {
        h |= s.shifted(-2, 0);
    }
    if xmin <= -3 {
        h |= s.shifted(-3, 0);
    }
    if xmax >= 1 {
        h |= s.shifted(1, 0);
    }
    if xmax >= 2 {
        h |= s.shifted(2, 0);
    }
    if xmax >= 3 {
        h |= s.shifted(3, 0);
    }
    let mut v = h;
    if ymin <= -1 {
        v |= h.shifted(0, -1);
    }
    if ymin <= -2 {
        v |= h.shifted(0, -2);
    }
    if ymin <= -3 {
        v |= h.shifted(0, -3);
    }
    if ymax >= 1 {
        v |= h.shifted(0, 1);
    }
    if ymax >= 2 {
        v |= h.shifted(0, 2);
    }
    if ymax >= 3 {
        v |= h.shifted(0, 3);
    }
    v
}

/// Returns whether `(P, r)` at `(x, y)` is collision-free in the single
/// lane of `b`, with every cell inside the plane.
///
/// Coordinates are lock coordinates, canonicalized exactly as
/// [`jade_core::data::place_mask`] canonicalizes them.
#[inline]
#[must_use]
pub fn check<const P: Piece>(b: &Plane<1>, x: i32, y: i32, r: usize) -> bool {
    if !(0..WIDTH).contains(&x) || !(0..LINES).contains(&y) {
        return false;
    }

    let rot = Rotation::from_u8(r as u8);
    let (ox, oy) = P.canonical_offset(rot);
    let rc = P.canonical_rotation(rot) as usize;
    let cx = x - ox;
    let cy = y - oy;
    if !(0..WIDTH).contains(&cx) || !(0..LINES).contains(&cy) {
        return false;
    }

    let lanes = b.0.to_array();
    let lane = lanes[0];
    let my = header::frame_min_y(P as usize, rc);
    let cells = [
        (0i32, -my),
        (i32::from(CELLS[P as usize][rc][0].0), i32::from(CELLS[P as usize][rc][0].1) - my),
        (i32::from(CELLS[P as usize][rc][1].0), i32::from(CELLS[P as usize][rc][1].1) - my),
        (i32::from(CELLS[P as usize][rc][2].0), i32::from(CELLS[P as usize][rc][2].1) - my),
    ];

    for (dx, dy) in cells {
        let col = cx + dx;
        let row = cy + dy;
        if !(0..WIDTH).contains(&col) || !(0..LINES).contains(&row) {
            return false;
        }
        if (lane >> (row * WIDTH + col) as u32) & 1 == 1 {
            return false;
        }
    }
    true
}

/// [`check`] through a precomputed [`usable_map`], with canonicalized
/// coordinates.
#[inline]
#[must_use]
pub fn check_fast<const P: Piece>(
    usable: &[Plane<1>; Rotation::NB],
    x: i32,
    y: i32,
    r: usize,
) -> bool {
    // The origin itself must be inside the 6-row working plane, not
    // just the canonical frame. Without this, kicks with negative dy
    // produce raw y that wraps silently in `Move::new`.
    if !(0..WIDTH).contains(&x) || !(0..LINES).contains(&y) {
        return false;
    }

    let rot = Rotation::from_u8(r as u8);
    let (dx, dy) = P.canonical_offset(rot);
    let rc = P.canonical_rotation(rot) as usize;
    let xc = x - dx;
    let yc = y - dy;

    if !(0..WIDTH).contains(&xc) || !(0..LINES).contains(&yc) {
        return false;
    }

    (usable[rc].0[0] >> (yc * WIDTH + xc) as u32) & 1 == 1
}

/// Rotates `mv` to `target`, applying the first valid kick.
/// Returns the original move when no kick succeeds or for `O`.
#[inline]
#[must_use]
pub fn apply_rotation<const P: Piece>(
    _board: &Plane<1>,
    usable: &[Plane<1>; Rotation::NB],
    mv: &Move,
    target: Rotation,
) -> Move {
    if mv.rotation() == target || mv.piece() == Piece::O {
        return *mv;
    }

    let kt = match P {
        Piece::I => &KICKS_I,
        _ => &KICKS_TJLSZ,
    };

    let lane = kt[mv.rotation() as usize][target as usize];
    for idx in 0..lane.1 {
        let dx = i32::from(lane.0[idx].0);
        let dy = i32::from(lane.0[idx].1);
        let nx = mv.x() + dx;
        let ny = mv.y() + dy;
        if check_fast::<P>(usable, nx, ny, target as usize) {
            return Move::new(P, target, nx, ny);
        }
    }

    *mv
}
