//! Ring buffer queue.

use std::mem::MaybeUninit;

const CAP: usize = 4096;
const MASK: usize = CAP - 1;

/// Ring buffer queue.
pub struct Fifo<T> {
    buf: [MaybeUninit<T>; CAP],
    front: usize,
    back: usize,
}

impl<T> Fifo<T> {
    /// Empty queue.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            // SAFETY: An uninitialized `[MaybeUninit<_>; CAP]` is valid.
            buf: unsafe { MaybeUninit::uninit().assume_init() },
            front: 0,
            back: 0,
        }
    }

    /// Append item.
    #[inline]
    pub const fn push_back(&mut self, val: T) {
        assert!(self.back - self.front != CAP, "queue full");
        self.buf[self.back & MASK].write(val);
        self.back = self.back.wrapping_add(1);
    }

    /// Remove first item.
    #[inline]
    pub const fn pop_front(&mut self) -> Option<T> {
        if self.front == self.back {
            return None;
        }
        let val = unsafe { self.buf[self.front & MASK].assume_init_read() };
        self.front = self.front.wrapping_add(1);
        Some(val)
    }

    /// True when empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.front == self.back
    }
}

impl<T> Default for Fifo<T> {
    fn default() -> Self {
        Self::new()
    }
}
