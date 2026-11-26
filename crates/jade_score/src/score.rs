//! Score state and the upper bound used for pruning.

use jade_core::spin::Spin;

use crate::event::Event;

/// Cells on the board after a perfect clear of a 4-row playfield pass.
/// A board with this many cells is a goal board for the 4-line target.
pub(super) const PC_CELLS: u32 = 40;

/// Cells in one piece.
pub(super) const CELLS_PER_PIECE: u32 = 4;

// Assumed upper bounds on the score of one move. They are not derived from
// `Event::score` in this module. If the scoring rules change, check these
// values. A value that is too low makes `remaining_bound` unsafe: the search
// could then discard the best path.


/// Back-to-back and combo state carried along a path.
#[derive(Clone, Copy)]
pub(super) struct ScoreState {
    pub(super) b2b: bool,
    pub(super) combo: Option<u32>,
}

impl ScoreState {
    /// Returns the state after a move, and the scoring event of that move.
    ///
    /// A move that clears no lines resets the combo and keeps `b2b`.
    #[inline]
    pub(super) fn apply(self, spin: Spin, lines: u32, pc: bool) -> (Self, Event) {
        if lines == 0 {
            let event = Event {
                b2b: false,
                combo: None,
                spin,
                lines,
                pc,
            };
            return (Self { b2b: self.b2b, combo: None }, event);
        }

        let difficult = spin != Spin::None || lines == 4;
        let combo = Some(self.combo.map_or(0, |c| c + 1));
        let event = Event {
            b2b: self.b2b && difficult,
            combo,
            spin,
            lines,
            pc,
        };
        (Self { b2b: difficult, combo }, event)
    }
}


