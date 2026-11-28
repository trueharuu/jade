use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::ops::Index;

/// Fast, fixed-capacity variable length list. This is designed for small,
/// `Copy` contexts.
pub struct List<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

#[allow(clippy::expl_impl_clone_on_copy)]
impl<T, const N: usize> Clone for List<T, N>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let mut out = Self {
            data: unsafe { MaybeUninit::uninit().assume_init() },
            len: self.len,
        };
        for i in 0..self.len {
            out.data[i] = MaybeUninit::new(unsafe { self.data[i].assume_init_ref().clone() });
        }
        out
    }
}

impl<T, const N: usize> List<T, N> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }

    pub const fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { self.data[index].assume_init_ref() })
    }

    pub const fn push(&mut self, value: T) {
        assert!(self.len < N);
        self.data[self.len] = MaybeUninit::new(value);
        self.len += 1;
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn shift(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let out = unsafe { self.data[0].assume_init_read() };

        let mut i = 1;
        while i < self.len {
            self.data[i - 1] = MaybeUninit::new(unsafe { self.data[i].assume_init_read() });
            i += 1;
        }

        self.len -= 1;
        Some(out)
    }

    pub const fn clear(&mut self) {
        self.len = 0;
    }

    pub const fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }

    pub const fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(unsafe { self.data[self.len].assume_init_read() })
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data[..self.len]
            .iter()
            .map(|x| unsafe { x.assume_init_ref() })
    }
}

impl<T, const N: usize> Extend<T> for List<T, N>
where
    T: Copy,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<T, const N: usize> Default for List<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> IntoIterator for List<T, N>
where
    T: Copy,
{
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let mut v = Vec::with_capacity(self.len);
        for i in 0..self.len {
            v.push(unsafe { self.data[i].assume_init_read() });
        }
        v.into_iter()
    }
}

impl<T: Copy, const N: usize> Copy for List<T, N> {}

impl<T, const N: usize> Index<usize> for List<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len);
        unsafe { self.data[index].assume_init_ref() }
    }
}

impl<T, const N: usize> Debug for List<T, N>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
