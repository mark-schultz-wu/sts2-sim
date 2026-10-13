//! The powers a fight's creatures carry.
//!
//! Each creature's powers stand in the order it gained them, which is the
//! order `IterateHookListeners` asks them in. Every power that arrives or
//! leaves goes through [`Powers`], so an implementor that keeps an index of
//! them keeps it true. Writing on a power's amount or its saved fields never
//! changes which hooks it overrides, so nothing reached through
//! [`Powers::of_mut`] moves the index either.
//!
//! [`Powers`] is the interface a fight uses. What stores the powers behind it
//! is the implementor's business, and each makes different questions cheap.

use sts2_core::{CombatId, Overrides, PowerTable as _, Rules};

use super::listen::HookCounts;
use super::live::{HeldPower, PowerInstance, PowerList};

/// The powers every creature in a fight carries.
pub(crate) trait Powers<V: Rules>: Clone + std::fmt::Debug {
    /// Nobody carrying anything.
    fn new() -> Self;

    /// `creature`'s powers, in the order it gained them.
    fn of(&self, creature: CombatId) -> impl Iterator<Item = &HeldPower<V>> + Clone + '_;

    /// `creature`'s powers, in order, to write on.
    fn of_mut(&mut self, creature: CombatId) -> impl Iterator<Item = &mut HeldPower<V>> + '_;

    /// Put `held` on `creature`, after everything it carries, as a new
    /// instance.
    fn push(&mut self, creature: CombatId, held: HeldPower<V>);

    /// Where the instance `instance` stands in `creature`'s order, if it is
    /// still on it.
    fn place_of(&self, creature: CombatId, instance: PowerInstance) -> Option<usize> {
        self.of(creature).position(|held| held.instance == instance)
    }

    /// Take the power at `at` in `creature`'s order off it.
    fn remove(&mut self, creature: CombatId, at: usize) -> HeldPower<V>;

    /// Take every power off `creature`.
    fn clear(&mut self, creature: CombatId);

    /// Whether any power in the fight overrides a hook in `hooks`.
    fn hear(&self, hooks: Overrides) -> bool;
}

/// The store a fight keeps its powers in.
pub(crate) type PowerStore<V> = Counted<V, PerCreature<V>>;

/// Where `creature`'s list stands, which is its id.
fn slot(creature: CombatId) -> usize {
    usize::from(creature.get())
}

/// A list per creature, by its id, and a scan to answer who listens.
#[derive(Debug)]
pub(crate) struct PerCreature<V: Rules> {
    lists: Vec<PowerList<V>>,
    /// The instance the next power to arrive is.
    next: PowerInstance,
}

impl<V: Rules> Clone for PerCreature<V> {
    fn clone(&self) -> Self {
        Self {
            lists: self.lists.clone(),
            next: self.next,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.lists.clone_from(&source.lists);
        self.next = source.next;
    }
}

impl<V: Rules> Powers<V> for PerCreature<V> {
    fn new() -> Self {
        Self {
            lists: Vec::new(),
            next: PowerInstance::default(),
        }
    }

    fn of(&self, creature: CombatId) -> impl Iterator<Item = &HeldPower<V>> + Clone + '_ {
        self.lists
            .get(slot(creature))
            .map_or(&[][..], |list| &list[..])
            .iter()
    }

    fn of_mut(&mut self, creature: CombatId) -> impl Iterator<Item = &mut HeldPower<V>> + '_ {
        self.lists
            .get_mut(slot(creature))
            .map_or(&mut [][..], |list| &mut list[..])
            .iter_mut()
    }

    fn push(&mut self, creature: CombatId, mut held: HeldPower<V>) {
        let at = slot(creature);
        if at >= self.lists.len() {
            self.lists.resize(at + 1, PowerList::new());
        }
        held.instance = self.next;
        self.next.0 += 1;
        self.lists[at].push(held);
    }

    fn remove(&mut self, creature: CombatId, at: usize) -> HeldPower<V> {
        self.lists[slot(creature)].remove(at)
    }

    fn clear(&mut self, creature: CombatId) {
        if let Some(list) = self.lists.get_mut(slot(creature)) {
            list.clear();
        }
    }

    fn hear(&self, hooks: Overrides) -> bool {
        self.lists
            .iter()
            .flat_map(|list| list.iter())
            .any(|held| held.power().overrides().intersects(hooks))
    }
}

/// Another store, and how many of its powers override each hook.
///
/// The counts move only as powers arrive and leave, which is all through
/// here, so [`Powers::hear`] is a lookup.
#[derive(Debug)]
pub(crate) struct Counted<V: Rules, P: Powers<V>> {
    inner: P,
    counts: HookCounts,
    kind: std::marker::PhantomData<V>,
}

impl<V: Rules, P: Powers<V>> Clone for Counted<V, P> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            counts: self.counts,
            kind: std::marker::PhantomData,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.inner.clone_from(&source.inner);
        self.counts = source.counts;
    }
}

impl<V: Rules, P: Powers<V>> Powers<V> for Counted<V, P> {
    fn new() -> Self {
        Self {
            inner: P::new(),
            counts: HookCounts::default(),
            kind: std::marker::PhantomData,
        }
    }

    fn of(&self, creature: CombatId) -> impl Iterator<Item = &HeldPower<V>> + Clone + '_ {
        self.inner.of(creature)
    }

    fn of_mut(&mut self, creature: CombatId) -> impl Iterator<Item = &mut HeldPower<V>> + '_ {
        self.inner.of_mut(creature)
    }

    fn push(&mut self, creature: CombatId, held: HeldPower<V>) {
        self.counts.add(held.power().overrides());
        self.inner.push(creature, held);
    }

    fn remove(&mut self, creature: CombatId, at: usize) -> HeldPower<V> {
        let held = self.inner.remove(creature, at);
        self.counts.remove(held.power().overrides());
        held
    }

    fn clear(&mut self, creature: CombatId) {
        for held in self.inner.of(creature) {
            self.counts.remove(held.power().overrides());
        }
        self.inner.clear(creature);
    }

    fn hear(&self, hooks: Overrides) -> bool {
        self.counts.hears(hooks)
    }
}
