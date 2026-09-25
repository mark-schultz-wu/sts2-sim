//! A set that iterates the way the game's does.
//!
//! `MapPoint.Children` and `MapPoint.parents` are `HashSet<MapPoint>` over a
//! type with no equality of its own. So .NET compares them by reference, and
//! walks them in the order they were added. Pruning walks children to
//! enumerate paths, and the order it finds them in decides which stretch is
//! shuffled first. A `HashSet` or a `BTreeSet` here generates another map.
//!
//! So this reproduces the entry array: additions land at the end, a removal
//! frees its slot, and the next addition takes the slot freed last. Iteration
//! walks the array and skips the free slots.

/// One entry, taken or free.
///
/// A free slot carries the one freed before it, which is the game's own free
/// list. It is a stack, so the newest free slot is the one reused next.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot<T> {
    Taken(T),
    Free { next: Option<usize> },
}

/// A set in insertion order, with the game's slot reuse.
///
/// Inline to four slots. A point on the map links to three at most, save
/// the first node and the boss, which link to a whole row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order<T> {
    slots: smallvec::SmallVec<[Slot<T>; 4]>,
    free: Option<usize>,
    taken: usize,
}

impl<T> Default for Order<T> {
    fn default() -> Self {
        Order {
            slots: smallvec::SmallVec::new(),
            free: None,
            taken: 0,
        }
    }
}

impl<T: Copy + PartialEq> Order<T> {
    pub fn new() -> Self {
        Order::default()
    }

    pub fn len(&self) -> usize {
        self.taken
    }

    pub fn contains(&self, value: T) -> bool {
        self.iter().any(|held| held == value)
    }

    /// Whether the value was not already there.
    pub fn insert(&mut self, value: T) -> bool {
        if self.contains(value) {
            return false;
        }
        self.taken += 1;
        match self.free {
            Some(at) => {
                let Slot::Free { next } = self.slots[at] else {
                    unreachable!("the free list holds only free slots")
                };
                self.free = next;
                self.slots[at] = Slot::Taken(value);
            }
            None => self.slots.push(Slot::Taken(value)),
        }
        true
    }

    /// Whether the value was there to remove.
    pub fn remove(&mut self, value: T) -> bool {
        let Some(at) = self
            .slots
            .iter()
            .position(|slot| matches!(slot, Slot::Taken(held) if *held == value))
        else {
            return false;
        };
        self.slots[at] = Slot::Free { next: self.free };
        self.free = Some(at);
        self.taken -= 1;
        true
    }

    /// In the order the game walks them.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Taken(value) => Some(*value),
            Slot::Free { .. } => None,
        })
    }

    /// The first, which is `Enumerable.First` over the same order.
    pub fn first(&self) -> Option<T> {
        self.iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additions_come_back_in_order() {
        let mut order = Order::new();
        for value in [7, 3, 9] {
            assert!(order.insert(value));
        }
        assert!(!order.insert(3));
        assert_eq!(order.iter().collect::<Vec<_>>(), [7, 3, 9]);
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn the_slot_freed_last_is_the_one_reused() {
        let mut order = Order::new();
        for value in [1, 2, 3, 4] {
            order.insert(value);
        }
        order.remove(2);
        order.remove(3);
        // Slot 2 was freed last, so 5 lands there and 6 lands in slot 1.
        order.insert(5);
        order.insert(6);
        assert_eq!(order.iter().collect::<Vec<_>>(), [1, 6, 5, 4]);
    }

    #[test]
    fn a_set_that_never_shrinks_grows_at_the_end() {
        let mut order = Order::new();
        for value in 0..4 {
            order.insert(value);
        }
        assert_eq!(order.first(), Some(0));
        assert!(order.remove(0));
        assert!(!order.remove(0));
        assert_eq!(order.first(), Some(1));
        assert_eq!(order.len(), 3);
    }
}
