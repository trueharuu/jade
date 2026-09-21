use std::mem::MaybeUninit;

const CAP: usize = 4096;
const MASK: usize = CAP - 1;

pub struct Queue<T> {
    buf: [MaybeUninit<T>; CAP],
    front: usize,
    back: usize,
}

impl<T> Queue<T> {
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            // SAFETY: An uninitialized `[MaybeUninit<_>; CAP]` is valid.
            buf: unsafe { MaybeUninit::uninit().assume_init() },
            front: 0,
            back: 0,
        }
    }

    #[inline(always)]
    pub const fn push_back(&mut self, val: T) {
        assert!(self.back - self.front != CAP, "queue full");
        self.buf[self.back & MASK].write(val);
        self.back = self.back.wrapping_add(1);
    }

    #[inline(always)]
    pub const fn pop_front(&mut self) -> Option<T> {
        if self.front == self.back {
            return None;
        }
        let val = unsafe { self.buf[self.front & MASK].assume_init_read() };
        self.front = self.front.wrapping_add(1);
        Some(val)
    }

    #[inline(always)]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.front == self.back
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}
