//! A list with a fixed capacity, held inline.

use std::mem::MaybeUninit;

/// Up to `N` items, inline, and `Copy` when its items are.
///
/// A fight clones at every node of a search. A list held here is part of that
/// one copy, with no allocation and no walk.
///
/// # Panics
///
/// Pushing past `N`. A capacity is a bound on what a fight holds, so reaching
/// it is a fight the port does not cover.
pub(crate) struct Bounded<T, const N: usize> {
    len: u16,
    items: [MaybeUninit<T>; N],
}

impl<T: Copy, const N: usize> Clone for Bounded<T, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Copy, const N: usize> Copy for Bounded<T, N> {}

impl<T: Copy, const N: usize> Bounded<T, N> {
    pub(crate) const fn new() -> Self {
        Self {
            len: 0,
            items: [MaybeUninit::uninit(); N],
        }
    }

    pub(crate) fn push(&mut self, item: T) {
        let at = usize::from(self.len);
        if at == N {
            unimplemented!("more than {N} of {}", std::any::type_name::<T>());
        }
        self.items[at] = MaybeUninit::new(item);
        self.len += 1;
    }

    pub(crate) fn insert(&mut self, at: usize, item: T) {
        let len = self.len();
        assert!(at <= len, "insert at {at} past a length of {len}");
        self.push(item);
        self.as_mut_slice()[at..].rotate_right(1);
    }

    pub(crate) fn remove(&mut self, at: usize) -> T {
        let item = self[at];
        self.as_mut_slice()[at..].rotate_left(1);
        self.len -= 1;
        item
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.retain_mut(|item| keep(item));
    }

    pub(crate) fn retain_mut(&mut self, mut keep: impl FnMut(&mut T) -> bool) {
        let mut to = 0;
        for at in 0..self.len() {
            let mut item = self[at];
            if keep(&mut item) {
                self.items[to] = MaybeUninit::new(item);
                to += 1;
            }
        }
        self.len = to as u16;
    }

    /// The items `source` holds, and not the capacity beside them.
    ///
    /// What a reused node copies. A copy of the whole list moves every slot.
    pub(crate) fn copy_from(&mut self, source: &Self) {
        let len = usize::from(source.len);
        self.items[..len].copy_from_slice(&source.items[..len]);
        self.len = source.len;
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    fn as_slice(&self) -> &[T] {
        let len = usize::from(self.len);
        // SAFETY: the first `len` items were each written by `push` or
        // `retain`, and nothing shortens `items` past them.
        unsafe { std::slice::from_raw_parts(self.items.as_ptr().cast::<T>(), len) }
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        let len = usize::from(self.len);
        // SAFETY: as `as_slice`.
        unsafe { std::slice::from_raw_parts_mut(self.items.as_mut_ptr().cast::<T>(), len) }
    }
}

impl<T: Copy, const N: usize> Default for Bounded<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy, const N: usize> std::ops::Deref for Bounded<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy, const N: usize> std::ops::DerefMut for Bounded<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T: Copy, const N: usize> Extend<T> for Bounded<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        for item in items {
            self.push(item);
        }
    }
}

impl<T: Copy, const N: usize> FromIterator<T> for Bounded<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut bounded = Self::new();
        bounded.extend(items);
        bounded
    }
}

impl<'a, T: Copy, const N: usize> IntoIterator for &'a Bounded<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Copy, const N: usize> IntoIterator for &'a mut Bounded<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: Copy + PartialEq, const N: usize> PartialEq for Bounded<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Eq, const N: usize> Eq for Bounded<T, N> {}

impl<T: Copy + std::fmt::Debug, const N: usize> std::fmt::Debug for Bounded<T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Bounded;

    #[test]
    fn a_list_keeps_its_order_through_every_edit() {
        let mut list: Bounded<u8, 8> = [1, 2, 3].into_iter().collect();
        list.insert(1, 9);
        assert_eq!(&*list, &[1, 9, 2, 3]);
        assert_eq!(list.remove(0), 1);
        list.retain(|&n| n != 2);
        assert_eq!(&*list, &[9, 3]);
    }

    #[test]
    #[should_panic(expected = "more than 2 of u8")]
    fn a_list_past_its_capacity_panics() {
        let _: Bounded<u8, 2> = [1, 2, 3].into_iter().collect();
    }
}
