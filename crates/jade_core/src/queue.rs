/// A fixed-order queue of pieces with an absolute cap at 11 pieces.
///
/// We store all pieces as their `u8` interpretation plus 1, such that `0` is
/// None. We choose 11 pieces as the maximum as it is the absolute maximum
/// amount of relevant pieces for a 10x4 field (10 pieces + 1 hold).
pub struct Queue([u8; 11]);

impl Queue {
    /// Creates a new empty queue.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Queue([0; 11])
    }

    /// Returns the number of pieces in the queue.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        let mut len = 0;
        while len < 11 && self.0[len] != 0 {
            len += 1;
        }

        len
    }

    /// Returns whether the queue is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
}
