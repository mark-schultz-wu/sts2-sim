//! What each player holds, by seat.
//!
//! `RunState.Players` is a list in seat order, and a fight's
//! `PlayerCombatState` hangs off each player. A run here keeps both as a
//! [`BySeat`].

use sts2_core::SeatIndex;

/// One `T` for each player, in seat order.
///
/// The host's sits inline, so a solo run holds its player's where it always
/// did and reaches it with one comparison. Every other seat's is in a list
/// after it.
#[derive(Debug)]
pub(crate) struct BySeat<T> {
    host: T,
    others: Vec<T>,
}

impl<T> BySeat<T> {
    /// The host's, and then the others', in seat order.
    pub(crate) fn new(host: T, others: Vec<T>) -> Self {
        Self { host, others }
    }

    /// How many players there are.
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        1 + self.others.len()
    }

    /// Whether any seat's answers `test`, the host's first.
    #[inline(always)]
    pub(crate) fn any(&self, mut test: impl FnMut(&T) -> bool) -> bool {
        // Checked first, since a solo run's list is empty, and asking an empty
        // list costs more than asking whether it is.
        test(&self.host) || (!self.others.is_empty() && self.others.iter().any(test))
    }

    /// The host's, which a solo run's player is.
    pub(crate) fn host(&self) -> &T {
        &self.host
    }

    /// Every seat but the host's, beside its seat, in order.
    pub(crate) fn others(&self) -> impl Iterator<Item = (SeatIndex, &T)> {
        (1..).map(SeatIndex).zip(&self.others)
    }
}

impl<T: Clone> Clone for BySeat<T> {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            others: self.others.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.host.clone_from(&source.host);
        self.others.clone_from(&source.others);
    }
}

impl<T> std::ops::Index<SeatIndex> for BySeat<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, seat: SeatIndex) -> &T {
        match seat.0 {
            0 => &self.host,
            other => &self.others[usize::from(other) - 1],
        }
    }
}

impl<T> std::ops::IndexMut<SeatIndex> for BySeat<T> {
    #[inline(always)]
    fn index_mut(&mut self, seat: SeatIndex) -> &mut T {
        match seat.0 {
            0 => &mut self.host,
            other => &mut self.others[usize::from(other) - 1],
        }
    }
}
