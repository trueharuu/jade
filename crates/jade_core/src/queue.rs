use std::fmt;
use std::hash::Hash;
use std::ops::Deref;
use std::str::FromStr;

use crate::piece::Piece;

/// A fixed-order queue of pieces with an absolute cap at [`CAP`] pieces.
/// 
/// When needed, a queue can be interpreted as a multiset of pieces.
///
/// The pieces are stored inline, so a queue costs one copy and no allocation.
/// A [`Queue`] is `Copy`, so it needs no `Drop` and passes through `&[Piece]`
/// signatures by deref.
#[derive(Clone, Copy)]
pub struct Queue {
    pieces: [Piece; CAP],
    len: u8,
}

/// The absolute maximum number of relevant pieces for a 10x4 field: 10 pieces
/// on the field plus 1 in the hold.
pub const CAP: usize = 11;

impl Queue {
    /// Creates a new empty queue.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pieces: [Piece::T; CAP],
            len: 0,
        }
    }

    /// Creates a queue holding `src`, in order.
    ///
    /// # Panics
    ///
    /// Panics if `src` is longer than [`CAP`].
    #[inline]
    #[must_use]
    pub const fn from_slice(src: &[Piece]) -> Self {
        let mut queue = Self::new();
        let mut i = 0;
        while i < src.len() {
            queue.push(src[i]);
            i += 1;
        }
        queue
    }

    /// Appends a piece to the back of the queue.
    ///
    /// # Panics
    ///
    /// Panics if the queue already holds [`CAP`] pieces.
    #[inline]
    pub const fn push(&mut self, piece: Piece) {
        assert!(self.len as usize != CAP, "queue full");
        self.pieces[self.len as usize] = piece;
        self.len += 1;
    }

    /// Shortens the queue to its first `n` pieces.
    #[inline]
    pub const fn truncate(&mut self, n: usize) {
        if n < self.len as usize {
            self.len = n as u8;
        }
    }

    /// Returns the first `n` pieces, or every piece when `n` is past the end.
    #[inline]
    #[must_use]
    pub const fn prefix(&self, n: usize) -> &[Piece] {
        let n = if n < self.len as usize {
            n
        } else {
            self.len as usize
        };
        self.pieces.split_at(n).0
    }

    /// Returns the pieces in order.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[Piece] {
        self.prefix(self.len as usize)
    }

    /// Returns the piece at `index`, or `None` when `index` is past the end.
    #[inline]
    #[must_use]
    pub const fn get(&self, index: usize) -> Option<Piece> {
        if index < self.len as usize {
            Some(self.pieces[index])
        } else {
            None
        }
    }

    /// Returns the first piece, or `None` when the queue is empty.
    #[inline]
    #[must_use]
    pub const fn first(&self) -> Option<Piece> {
        self.get(0)
    }

    /// Returns an iterator over the pieces, in order.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Piece> {
        self.as_slice().iter()
    }

    /// Returns the number of pieces in the queue.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns whether the queue is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns whether the piece `p` is present.
    #[inline]
    #[must_use]
    pub const fn contains(&self, p: Piece) -> bool {
        let mut i = 0;
        while i < self.len as usize {
            if self.pieces[i] as u8 == p as u8 {
                return true;
            }
            i += 1;
        }

        false
    }

    /// Consumes `slice` from the queue, removing the elements from left-to-right.
    pub const fn consume(self, slice: &[Piece]) -> Self {
        let mut queue = self;
        let mut i = 0;
        while i < slice.len() {
            let mut j = 0;
            while j < queue.len as usize {
                if queue.pieces[j] as u8 == slice[i] as u8 {
                    // Remove the piece by shifting the tail left.
                    let mut k = j + 1;
                    while k < queue.len as usize {
                        queue.pieces[k - 1] = queue.pieces[k];
                        k += 1;
                    }
                    queue.len -= 1;
                    break;
                }
                j += 1;
            }
            i += 1;
        }
        queue
    }
}

impl Default for Queue {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Queue {
    type Target = [Piece];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[Piece]> for Queue {
    #[inline]
    fn as_ref(&self) -> &[Piece] {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a Queue {
    type Item = &'a Piece;
    type IntoIter = std::slice::Iter<'a, Piece>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Queue {
    type Item = Piece;
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            pieces: self.pieces,
            len: self.len as usize,
            front: 0,
        }
    }
}

/// An owning iterator over the live pieces of a [`Queue`].
///
/// The array is moved into the iterator, so the iterator yields only the
/// prefix that was live and skips the unused tail.
pub struct IntoIter {
    pieces: [Piece; CAP],
    len: usize,
    front: usize,
}

impl Iterator for IntoIter {
    type Item = Piece;

    #[inline]
    fn next(&mut self) -> Option<Piece> {
        if self.front == self.len {
            return None;
        }
        let piece = self.pieces[self.front];
        self.front += 1;
        Some(piece)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len - self.front;
        (n, Some(n))
    }
}

impl ExactSizeIterator for IntoIter {}

impl FromIterator<Piece> for Queue {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Piece>>(iter: I) -> Self {
        let mut queue = Self::new();
        queue.extend(iter);
        queue
    }
}

impl Extend<Piece> for Queue {
    #[inline]
    fn extend<I: IntoIterator<Item = Piece>>(&mut self, iter: I) {
        for piece in iter {
            self.push(piece);
        }
    }
}

impl PartialEq for Queue {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for Queue {}

impl PartialOrd for Queue {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Compares only the live prefix. The order must match the order of `Vec`, as
/// `Vec::cmp` delegates to the same slice comparison.
impl Ord for Queue {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

/// Hashes only the live prefix. This matches the hash of a `Vec`, as `Vec`
/// delegates to the same slice hash.
impl Hash for Queue {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

/// Prints the pieces in order, with no separator.
impl fmt::Display for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &piece in self {
            write!(f, "{piece}")?;
        }
        Ok(())
    }
}

/// Prints the live prefix as a list. The unused tail is not printed.
impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a> Extend<&'a Piece> for Queue {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a Piece>>(&mut self, iter: I) {
        for &piece in iter {
            self.push(piece);
        }
    }
}

impl FromStr for Queue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut queue = Queue::new();
        for c in s.chars() {
            let piece =
                Piece::from_str(&c.to_string()).map_err(|_| format!("invalid piece: {c}"))?;
            queue.push(piece);
        }
        Ok(queue)
    }
}