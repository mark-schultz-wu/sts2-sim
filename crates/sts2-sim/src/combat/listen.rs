//! Every listener in a fight, asked in the order the game asks them.
//!
//! `Hook.AfterX` walks `CombatState.IterateHookListeners`: each creature in
//! turn, allies before enemies, its powers first. A monster's creature then
//! brings the monster's own model. The player's brings its belts, its
//! potions, and every card in its five piles with what each card carries.
//! The run's modifiers come last. [`listed`] hands that order out.
//!
//! Only a listener whose class overrides a hook is asked it. The game takes
//! the list before anyone is asked, and skips one that has left the fight by
//! the time the walk reaches it.

use smallvec::SmallVec;
use sts2_core::{
    CardId, CardTable as _, CombatId, DecisionHook, EventHook, HookEffect, ModelHook, ModelId,
    MonsterTable as _, Overrides, PowerTable as _, RelicTable as _, Rules,
};

use super::live::{Fighter, Heard, HeldPower, Live};
use super::powers::Powers as _;
use crate::run::{Belt, Belts};

/// One listener, by what names it rather than by a borrow of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Listener<V: Rules> {
    /// The power instance `instance` on the creature `owner`.
    Power {
        owner: CombatId,
        instance: super::live::PowerInstance,
        power: V::Power,
    },
    /// The relic in slot `slot` of `seat`'s belt.
    Relic {
        seat: sts2_core::SeatIndex,
        slot: u8,
    },
    /// The monster model of the creature `creature`.
    Monster {
        creature: CombatId,
        monster: V::Monster,
    },
    Card {
        id: CardId<V>,
    },
    /// A modifier the run was started with.
    Modifier {
        modifier: sts2_core::RunModifier<V>,
    },
}

/// The seat whose player `listener` acts for: a relic's or a card's owner,
/// or the player a power's creature is or belongs to. Nothing for a monster
/// or a modifier, which belong to no player.
pub(crate) fn owner<V: Rules>(
    live: &Live<V>,
    listener: Listener<V>,
) -> Option<sts2_core::SeatIndex> {
    match listener {
        Listener::Relic { seat, .. } => Some(seat),
        Listener::Power { owner, .. } => live.seat_of(owner),
        Listener::Card { id } => live
            .players
            .seated()
            .find(|(_, player)| player.cards.named(id).is_some())
            .map(|(seat, _)| seat),
        Listener::Monster { .. } | Listener::Modifier { .. } => None,
    }
}

/// How many of a container's listeners override each hook.
///
/// A container that holds listeners keeps one, and moves it only where a
/// listener arrives or leaves. A listener's class fixes which hooks it
/// overrides, so nothing else moves it.
///
/// The set of hooks with a count above zero sits beside the counts, so
/// whether anyone listens is one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HookCounts {
    counts: [u16; ModelHook::ALL.len()],
    heard: Overrides,
}

impl Default for HookCounts {
    fn default() -> Self {
        HookCounts {
            counts: [0; ModelHook::ALL.len()],
            heard: Overrides::default(),
        }
    }
}

impl HookCounts {
    /// A listener that overrides `hooks` arrives.
    pub(crate) fn add(&mut self, hooks: Overrides) {
        for at in hooks.places() {
            self.counts[at] += 1;
        }
        self.heard = self.heard.union(hooks);
    }

    /// A listener that overrides `hooks` leaves.
    pub(crate) fn remove(&mut self, hooks: Overrides) {
        for at in hooks.places() {
            self.counts[at] -= 1;
            if self.counts[at] == 0 {
                self.heard = self.heard.without(ModelHook::ALL[at]);
            }
        }
    }

    /// Whether any listener overrides a hook in `hooks`.
    pub(crate) fn hears(&self, hooks: Overrides) -> bool {
        self.heard.intersects(hooks)
    }

    /// Every listener stops hearing, as a dead player's hooks do.
    pub(crate) fn deafen(&mut self) {
        self.heard = Overrides::default();
    }
}

/// What a walk reads without writing: the fight, and the belt its player
/// carries.
pub(crate) struct Walk<'a, V: Rules> {
    pub(crate) live: &'a Live<V>,
    pub(crate) belts: &'a Belts<V>,
}

impl<V: Rules> Clone for Walk<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Walk<'_, V> {}

/// The first thing a listener asks for when `hook` is raised, and the
/// listener that asked it, walking the listeners until one asks.
///
/// For a hook whose listeners write only on themselves, where an answer is a
/// path nobody has ported. Each listener is asked as the walk reaches it, as
/// any walk asks, and a power's own decrement is carried out in its turn.
///
/// # Panics
///
/// A hook that is not one `AbstractModel` declares, which is asked of one
/// model directly rather than raised.
#[inline(always)]
pub(crate) fn first_answer<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut Belts<V>,
    hook: EventHook<V>,
) -> Option<(Listener<V>, HookEffect<V>)> {
    let walker = Walker::begin(live, belts, hook)?;
    first_of_walk(seat, live, belts, walker)
}

/// [`first_answer`] once something hears the hook.
#[inline(never)]
fn first_of_walk<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut Belts<V>,
    mut walker: Walker<V>,
) -> Option<(Listener<V>, HookEffect<V>)> {
    let mut asked = sts2_core::HookEffects::new();
    while let Some(listener) = walker.next(seat, live, belts, &mut asked) {
        if let Some(&effect) = asked.first() {
            return Some((listener, effect));
        }
    }
    None
}

/// How a walk finds its listeners: the list `IterateHookListeners` takes as
/// the walk begins, handed out one at a time.
///
/// A cursor only says which listener comes next. [`Walker`] asks whether it
/// is still in the fight, and asks it. A cursor need take no list at all,
/// so long as it hands out what the list held, in its order.
pub(crate) trait Cursor<V: Rules>: std::fmt::Debug + Clone + PartialEq {
    /// The listeners that override `hook`, as the fight stands.
    fn begin(live: &Live<V>, belts: &Belts<V>, hook: ModelHook) -> Self;

    /// The next listener the list held, or nothing past its end.
    fn next(&mut self, live: &Live<V>, belts: &Belts<V>) -> Option<Snapped<V>>;
}

/// The cursor every walk uses.
pub(crate) type Active<V> = Snapshot<V>;

/// `Hook`'s walk over a hook's listeners, each asked as the walk reaches it.
///
/// What a listener asks for lands before the next is asked, which the caller
/// sees to between calls to [`Walker::next`]. The walk asks `Contains(item)`
/// of each, so one whose creature or card left the fight is skipped.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Walker<V: Rules, C: Cursor<V> = Active<V>> {
    hook: EventHook<V>,
    cursor: C,
    /// What a listener asked from its decrement on, which waits for what
    /// came before to land, and the listener that asked it.
    /// A `Vec`, as nearly every walk leaves it empty and moves it often.
    pending: Vec<HookEffect<V>>,
    pending_from: Option<Listener<V>>,
}

/// The reference cursor: the whole list, taken as the walk begins.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Snapshot<V: Rules> {
    listeners: SmallVec<[Snapped<V>; 8]>,
    next: usize,
}

/// One listener as a cursor found it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Snapped<V: Rules> {
    Power {
        owner: CombatId,
        instance: super::live::PowerInstance,
        power: V::Power,
        /// Where its owner and it stood as the walk began, which is where
        /// they stand still unless a listener moved something.
        creature_at: u8,
        at: u8,
    },
    Monster {
        creature: CombatId,
        monster: V::Monster,
    },
    Relic {
        seat: sts2_core::SeatIndex,
        slot: u8,
        relic: V::Relic,
    },
    Card {
        seat: sts2_core::SeatIndex,
        id: CardId<V>,
    },
    Modifier {
        modifier: sts2_core::RunModifier<V>,
    },
}

impl<V: Rules> Walker<V> {
    /// The walk `Hook` makes over the listeners to `hook`, as the fight
    /// stands. `Hook` hands a hook nobody hears no listeners, and then there
    /// is no walk. The fight's counts say so before any list is taken.
    ///
    /// # Panics
    ///
    /// A hook that is not one `AbstractModel` declares.
    #[inline(always)]
    pub(crate) fn begin(live: &Live<V>, belts: &Belts<V>, hook: EventHook<V>) -> Option<Self> {
        let model_hook = hook
            .model_hook()
            .unwrap_or_else(|| unreachable!("{hook:?} is asked of one model, not raised"));
        let heard = Kinds::hearing(live, belts, Overrides::of(model_hook)).is_some();
        heard.then(|| Walker {
            hook,
            cursor: Active::begin(live, belts, model_hook),
            pending: Vec::new(),
            pending_from: None,
        })
    }
}

impl<V: Rules> Cursor<V> for Snapshot<V> {
    /// # Panics
    ///
    /// A potion hearing `hook`.
    fn begin(live: &Live<V>, belts: &Belts<V>, model_hook: ModelHook) -> Self {
        Snapshot {
            listeners: snapshot(live, belts, model_hook),
            next: 0,
        }
    }

    fn next(&mut self, _: &Live<V>, _: &Belts<V>) -> Option<Snapped<V>> {
        let snapped = self.listeners.get(self.next).copied()?;
        self.next += 1;
        Some(snapped)
    }
}

/// One listener `IterateHookListeners` lists, as [`listed`] finds it, with
/// every hook its class overrides.
pub(crate) enum Found<'a, V: Rules> {
    /// The power `held`, at `at` among its owner's, on the creature
    /// `creature` at `creature_at` among the fight's.
    Power {
        creature_at: usize,
        creature: &'a Fighter<V>,
        at: usize,
        held: &'a HeldPower<V>,
        overrides: Overrides,
    },
    /// The monster model of `creature`.
    Monster {
        creature: &'a Fighter<V>,
        monster: V::Monster,
        overrides: Overrides,
    },
    /// The relic in belt slot `slot`.
    /// The relic in slot `slot` of `seat`'s belt.
    Relic {
        seat: sts2_core::SeatIndex,
        slot: usize,
        overrides: Overrides,
    },
    Potion {
        potion: V::Potion,
        overrides: Overrides,
    },
    /// The copy `id`, with what its card, its affliction and its
    /// enchantment override together.
    Card {
        seat: sts2_core::SeatIndex,
        id: CardId<V>,
        overrides: Overrides,
    },
    Modifier {
        modifier: sts2_core::RunModifier<V>,
        overrides: Overrides,
    },
}

/// What [`listed`] looks for: one hook, as a walk raises, or a set of them,
/// as a number's phases are asked together. One hook reads the belt's
/// listening slots for it directly.
trait Sought: Copy {
    /// Whether a listener that overrides `overrides` is sought.
    fn sought_by(self, overrides: Overrides) -> bool;
    /// The belt's slots whose relic is sought.
    fn relic_slots<V: Rules>(self, belt: &Belt<V>) -> u64;
    fn hooks(self) -> Overrides;
}

/// One hook, beside the set holding it alone.
#[derive(Clone, Copy)]
struct OneHook {
    hook: ModelHook,
    hooks: Overrides,
}

impl Sought for OneHook {
    #[inline(always)]
    fn sought_by(self, overrides: Overrides) -> bool {
        overrides.intersects(self.hooks)
    }

    #[inline(always)]
    fn relic_slots<V: Rules>(self, belt: &Belt<V>) -> u64 {
        belt.hearing_hook(self.hook)
    }

    #[inline(always)]
    fn hooks(self) -> Overrides {
        self.hooks
    }
}

impl Sought for Overrides {
    #[inline(always)]
    fn sought_by(self, overrides: Overrides) -> bool {
        overrides.intersects(self)
    }

    #[inline(always)]
    fn relic_slots<V: Rules>(self, belt: &Belt<V>) -> u64 {
        belt.hearing(self)
    }

    #[inline(always)]
    fn hooks(self) -> Overrides {
        self
    }
}

/// `CombatState.IterateHookListeners`: every listener that overrides what is
/// `sought`, handed to `found` in the game's order.
///
/// That is each creature's powers, then its monster. For the player, its
/// belts but a melted one, its potions, then its cards in `AllPiles` order.
/// The run's modifiers come last. A kind `kinds` rules out is not looked
/// through.
#[inline(always)]
fn listed<'a, V: Rules, S: Sought>(
    live: &'a Live<V>,
    belts: &Belts<V>,
    sought: S,
    kinds: Kinds,
    mut found: impl FnMut(Found<'a, V>),
) {
    let hooks = sought.hooks();
    for (creature_at, creature) in live.creatures.iter().enumerate() {
        // `Contains`: a dead player's `DeactivateHooks` takes its own powers
        // out, as it does its belts, potions and cards.
        if kinds.powers && !(live.lost && matches!(creature.model, Some(ModelId::Character(_)))) {
            for (at, held) in live.powers.of(creature.id).enumerate() {
                let overrides = held.power().overrides();
                if sought.sought_by(overrides) {
                    found(Found::Power {
                        creature_at,
                        creature,
                        at,
                        held,
                        overrides,
                    });
                }
            }
        }
        if kinds.monsters
            && let Some(monster) = creature.carrier().monster
            && sought.sought_by(monster.overrides())
        {
            found(Found::Monster {
                creature,
                monster,
                overrides: monster.overrides(),
            });
        }
        let Some(ModelId::Character(_)) = creature.model else {
            continue;
        };
        // A fight adds its players first, so a player's creature is its seat.
        let seat = sts2_core::SeatIndex(creature.id.get());
        if kinds.belt {
            // The listening slots alone, in slot order.
            let belt = &belts[seat];
            let mut left = sought.relic_slots(belt);
            while left != 0 {
                let slot = left.trailing_zeros() as usize;
                left &= left - 1;
                found(Found::Relic {
                    seat,
                    slot,
                    overrides: belt[slot].relic.overrides(),
                });
            }
        }
        if kinds.others {
            listed_potions(seat, live, hooks, &mut found);
        }
        if kinds.cards {
            for (id, overrides) in live.players[seat].cards.listening(hooks) {
                found(Found::Card {
                    seat,
                    id,
                    overrides,
                });
            }
        }
    }
    if kinds.others {
        listed_modifiers(live, hooks, &mut found);
    }
}

/// [`listed`] over the belt's potions that override one of `hooks`. Out of
/// line, since few fights hold a potion that hears anything.
#[cold]
#[inline(never)]
fn listed_potions<'a, V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &'a Live<V>,
    hooks: Overrides,
    found: &mut impl FnMut(Found<'a, V>),
) {
    for &potion in live.players[seat].potions.held() {
        let overrides = sts2_core::PotionTable::overrides(potion);
        if overrides.intersects(hooks) {
            found(Found::Potion { potion, overrides });
        }
    }
}

/// [`listed`] over the run's modifiers that override one of `hooks`. Out of
/// line, since few runs carry a modifier a fight asks.
#[cold]
#[inline(never)]
fn listed_modifiers<'a, V: Rules>(
    live: &'a Live<V>,
    hooks: Overrides,
    found: &mut impl FnMut(Found<'a, V>),
) {
    for (modifier, overrides) in live.modifiers.hearing(hooks) {
        found(Found::Modifier {
            modifier,
            overrides,
        });
    }
}

/// [`listed`] over the listeners that override `model_hook`, each as a walk
/// finds it again.
///
/// # Panics
///
/// A potion hearing `model_hook`.
fn snapshot<V: Rules>(
    live: &Live<V>,
    belts: &Belts<V>,
    model_hook: ModelHook,
) -> SmallVec<[Snapped<V>; 8]> {
    let hooks = Overrides::of(model_hook);
    let mut listeners = SmallVec::new();
    let Some(kinds) = Kinds::hearing(live, belts, hooks) else {
        return listeners;
    };
    listed(
        live,
        belts,
        OneHook {
            hook: model_hook,
            hooks,
        },
        kinds,
        #[inline(always)]
        |found| {
            listeners.push(match found {
                Found::Power {
                    creature_at,
                    creature,
                    at,
                    held,
                    ..
                } => Snapped::Power {
                    owner: creature.id,
                    instance: held.instance,
                    power: held.power(),
                    creature_at: creature_at as u8,
                    at: at as u8,
                },
                Found::Monster {
                    creature, monster, ..
                } => Snapped::Monster {
                    creature: creature.id,
                    monster,
                },
                Found::Relic { seat, slot, .. } => Snapped::Relic {
                    seat,
                    slot: slot as u8,
                    relic: belts[seat][slot].relic,
                },
                Found::Potion { potion, .. } => {
                    unimplemented!("{potion:?} hearing {model_hook:?}")
                }
                Found::Card { seat, id, .. } => Snapped::Card { seat, id },
                Found::Modifier { modifier, .. } => Snapped::Modifier { modifier },
            });
        },
    );
    listeners
}

impl<V: Rules, C: Cursor<V>> Walker<V, C> {
    /// The hook as the listeners still to come are handed it, where what it
    /// carries is read off the fight as each is reached.
    pub(crate) fn rehook(&mut self, hook: EventHook<V>) {
        self.hook = hook;
    }

    /// The next listener in the walk that asks for anything, with what it
    /// asks for in `asked`. A power's `PowerCmd.Decrement` of itself is
    /// carried out here, in its turn. What it asked before the decrement
    /// comes back first. The rest comes on the next call, after what its
    /// `AfterRemoved` asks for. That call comes back even with nothing asked,
    /// so the caller can read the fight again.
    ///
    /// # Panics
    ///
    /// A belt that changed under the walk.
    pub(crate) fn next(
        &mut self,
        seat: sts2_core::SeatIndex,
        live: &mut Live<V>,
        belts: &mut Belts<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) -> Option<Listener<V>> {
        asked.clear();
        if let Some(listener) = self.pending_from {
            if let Listener::Power {
                owner, instance, ..
            } = listener
                && self.pending.first() == Some(&HookEffect::Decrement)
            {
                self.pending.remove(0);
                *asked = decrement::<V>(seat, live, owner, instance);
            }
            self.take_pending(asked);
            return Some(listener);
        }
        let hook = self.hook;
        while let Some(snapped) = self.cursor.next(live, belts) {
            let listener = match snapped {
                Snapped::Power {
                    owner,
                    instance,
                    power,
                    creature_at,
                    at,
                } => {
                    // `Contains`: the power's owner is still in the fight,
                    // and a player's own is active for hooks, which the
                    // player's death ends with `DeactivateHooks`. The fight
                    // is lost from then.
                    let fighter = match live.creatures.get(usize::from(creature_at)) {
                        Some(creature) if creature.id == owner => creature,
                        _ => match live.creatures.iter().find(|creature| creature.id == owner) {
                            Some(creature) => creature,
                            None => continue,
                        },
                    };
                    if live.lost && matches!(fighter.model, Some(sts2_core::ModelId::Character(_)))
                    {
                        continue;
                    }
                    let carrier = fighter.carrier();
                    let at = match live.powers.of(owner).nth(usize::from(at)) {
                        Some(held) if held.instance == instance => Some(usize::from(at)),
                        _ => live.powers.place_of(owner, instance),
                    };
                    match at {
                        Some(at) => live.ask_power_as(Some(&*belts), carrier, at, hook, asked),
                        // Taken off since the walk began: asked as a model on
                        // its own, as `Hook` asks every item of its list.
                        None => {
                            if !live.ask_detached_as(Some(&*belts), carrier, instance, hook, asked)
                            {
                                continue;
                            }
                        }
                    }
                    Listener::Power {
                        owner,
                        instance,
                        power,
                    }
                }
                Snapped::Monster { creature, monster } => {
                    if !live.creatures.iter().any(|fighter| fighter.id == creature) {
                        continue;
                    }
                    let Some(machine) = live
                        .machines
                        .iter_mut()
                        .find(|machine| machine.creature == creature)
                    else {
                        unreachable!("a monster in the fight has its graph")
                    };
                    V::monster_event_hook(monster, &mut machine.state, creature, hook, asked);
                    Listener::Monster { creature, monster }
                }
                Snapped::Relic { seat, slot, relic } => {
                    // `Contains`: its owner is active for hooks.
                    if live.lost {
                        continue;
                    }
                    if belts[seat].get(usize::from(slot)).map(|held| held.relic) != Some(relic) {
                        unimplemented!("the belt changing under a walk over {hook:?}")
                    }
                    let (state, belt) = belts[seat].state_beside(usize::from(slot));
                    V::relic_event_hook(state, belt, live.relic_owner(seat), hook, asked);
                    Listener::Relic { seat, slot }
                }
                Snapped::Card { seat, id } => {
                    // `Contains`: still in the fight, and its owner active for
                    // hooks.
                    if live.lost || !ask_card(seat, live, id, hook, asked) {
                        continue;
                    }
                    Listener::Card { id }
                }
                Snapped::Modifier { modifier } => {
                    ask_modifier(modifier, hook, asked);
                    Listener::Modifier { modifier }
                }
            };
            if asked.is_empty() {
                continue;
            }
            if let Listener::Power { .. } = listener
                && let Some(at) = asked.iter().position(|e| *e == HookEffect::Decrement)
            {
                self.pending.extend(asked.drain(at..));
                self.pending_from = Some(listener);
                if asked.is_empty() {
                    return self.next(seat, live, belts, asked);
                }
            }
            return Some(listener);
        }
        None
    }

    /// Move what waits into `asked`, up to the next decrement.
    fn take_pending(&mut self, asked: &mut sts2_core::HookEffects<V>) {
        let upto = self
            .pending
            .iter()
            .position(|e| *e == HookEffect::Decrement)
            .unwrap_or(self.pending.len());
        asked.extend(self.pending.drain(..upto));
        if self.pending.is_empty() {
            self.pending_from = None;
        }
    }
}

/// A walk partway through what one listener asked, which can park on a
/// question one of its effects asks and go on after the answer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Walking<V: Rules> {
    walker: Walker<V>,
    /// The listener whose effects these are, and what it asked still to
    /// carry out.
    listener: Option<Listener<V>>,
    rest: sts2_core::HookEffects<V>,
}

impl<V: Rules> Walking<V> {
    /// [`Walker::begin`], to carry out one effect at a time.
    #[inline(always)]
    pub(crate) fn begin(live: &Live<V>, belts: &Belts<V>, hook: EventHook<V>) -> Option<Self> {
        Walker::begin(live, belts, hook).map(Self::new)
    }

    /// [`Walker::rehook`]: the hook as the listeners still to come are
    /// handed it.
    pub(crate) fn rehook(&mut self, hook: EventHook<V>) {
        self.walker.rehook(hook);
    }

    fn new(walker: Walker<V>) -> Self {
        Self {
            walker,
            listener: None,
            rest: sts2_core::HookEffects::new(),
        }
    }

    /// The next effect to carry out, and the listener that asked it. The next
    /// listener is asked only once the last one's effects are out.
    pub(crate) fn next(
        &mut self,
        seat: sts2_core::SeatIndex,
        live: &mut Live<V>,
        belts: &mut Belts<V>,
    ) -> Option<(Listener<V>, HookEffect<V>)> {
        loop {
            if let Some(listener) = self.listener
                && !self.rest.is_empty()
            {
                return Some((listener, self.rest.remove(0)));
            }
            self.listener = Some(self.walker.next(seat, live, belts, &mut self.rest)?);
        }
    }
}

/// `hook` asked of the copy `id` and of what it carries. The card's own
/// override comes first, then its affliction's, then its enchantment's, as
/// `IterateHookListeners` lists them. Whether the fight still holds the
/// copy.
///
/// # Panics
///
/// A hook that is not one `AbstractModel` declares.
#[inline(always)]
fn ask_card<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &Live<V>,
    id: CardId<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) -> bool {
    let Some(card) = live.players[seat].cards.named(id) else {
        return false;
    };
    // A hook raised for one player reaches another player's card, which
    // returns at once: each of the game's checks `player == Owner`.
    if let EventHook::BeforeHandDraw { player, .. }
    | EventHook::EnergyReset { player, .. }
    | EventHook::EnergyResetLate { player, .. }
    | EventHook::PlayerTurnStart { player, .. }
    | EventHook::PlayerTurnStartLate { player, .. }
    | EventHook::AutoPrePlayPhaseEarly { player, .. }
    | EventHook::AutoPrePlayPhase { player, .. }
    | EventHook::AutoPrePlayPhaseLate { player, .. }
    | EventHook::AutoPostPlayPhase { player, .. }
    | EventHook::StarsSpent { player, .. }
    | EventHook::HandEmptied { player, .. } = hook
        && player != seat.creature()
    {
        return false;
    }
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one model, not raised"));
    let ident = card.face.ident;
    if ident.card.overrides().contains(model_hook) {
        V::card_event_hook(
            ident.card,
            id,
            live.players[seat].cards.pile_of(id),
            &card,
            live.history(),
            hook,
            asked,
        );
    }
    if let Some(affliction) = ident.affliction
        && sts2_core::AfflictionTable::overrides(affliction).contains(model_hook)
    {
        V::affliction_event_hook(affliction, id, hook, asked);
    }
    if let Some(enchanted) = ident.enchantment
        && sts2_core::EnchantmentTable::overrides(enchanted.enchantment).contains(model_hook)
    {
        V::enchantment_event_hook(enchanted, id, hook, asked);
    }
    true
}

/// `PowerCmd.Decrement` of the instance `instance` on `owner`, and its
/// removal at nothing. What its `AfterRemoved` asks for comes back.
fn decrement<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    owner: CombatId,
    instance: super::live::PowerInstance,
) -> sts2_core::HookEffects<V> {
    let Some(held) = live
        .powers
        .of_mut(owner)
        .find(|held| held.instance == instance)
    else {
        // One taken off already ticks down on its own model, and comes off
        // nothing more.
        if let Some((_, held)) = live
            .detached
            .iter_mut()
            .find(|(holder, held)| *holder == owner && held.instance == instance)
            && held.amount.count().is_some()
        {
            held.amount = held.amount.ticked();
        }
        return sts2_core::HookEffects::new();
    };
    if held.amount.count().is_some() {
        held.amount = held.amount.ticked();
    }
    if !held.amount.spent() {
        return sts2_core::HookEffects::new();
    }
    match live.powers.place_of(owner, instance) {
        Some(at) => live.remove_power_at_asking(seat, owner, at),
        None => sts2_core::HookEffects::new(),
    }
}

/// Which kinds of listener hear a hook, and so which a walk looks through.
#[derive(Clone, Copy)]
struct Kinds {
    powers: bool,
    monsters: bool,
    belt: bool,
    cards: bool,
    /// A potion or a run's modifier, which few fights hold one of that
    /// hears anything.
    others: bool,
}

/// `Hook.ModifyUnblockedDamageTarget`: the creature that takes the damage
/// past `target`'s block.
///
/// Each listener is handed the creature the one before it chose, and a power
/// that answers yes chooses the creature carrying it.
///
/// # Panics
///
/// A listener other than a power, which nothing ported is.
pub(crate) fn unblocked_damage_target<V: Rules>(
    fight: Walk<'_, V>,
    target: CombatId,
    powered: bool,
) -> CombatId {
    let hooks = Overrides::of(ModelHook::ModifyUnblockedDamageTarget);
    let Some(kinds) = Kinds::hearing(fight.live, fight.belts, hooks) else {
        return target;
    };
    if kinds.monsters || kinds.belt || kinds.cards || kinds.others {
        unimplemented!("a listener other than a power choosing where a hit lands");
    }
    let mut chosen = target;
    let onlookers = super::live::Onlookers {
        creatures: &fight.live.creatures,
        powers: &fight.live.powers,
        kept: &fight.live.kept,
        belts: Some(fight.belts),
        attack: &fight.live.attack_results,
    };
    listed(fight.live, fight.belts, hooks, kinds, |found| {
        let Found::Power { creature, held, .. } = found else {
            unreachable!("only powers hear it, as asked above")
        };
        let hook = DecisionHook::TakesUnblockedDamage {
            target: chosen,
            powered,
            pet_of: creature.owner,
            alive: creature.alive,
        };
        let listener = held.listener(creature.carrier(), fight.live.history(), &onlookers);
        if V::decide_hook(listener, hook).is_decision_and(|takes| takes) {
            chosen = creature.id;
        }
    });
    chosen
}

/// Whether anything in the fight overrides `hook`.
///
/// For a hook the fight raises nowhere yet. A listener that answers one is a
/// path nobody has ported, and asking this is how it stops by name.
pub(crate) fn heard<V: Rules>(live: &Live<V>, belts: &Belts<V>, hook: ModelHook) -> bool {
    heard_any::<V>(live, belts, Overrides::of(hook))
}

/// Whether anything in the fight overrides any of `hooks`.
///
/// The kinds [`Kinds::hearing`] weighs, asked in turn and no further than
/// the first that hears.
pub(crate) fn heard_any<V: Rules>(live: &Live<V>, belts: &Belts<V>, hooks: Overrides) -> bool {
    live.powers.hear(hooks)
        || live.creatures.hear(hooks)
        || belts.any(|belt| belt.hear(hooks))
        || live.players.any(|player| player.cards.hears(hooks))
        || live
            .players
            .any(|player| player.potions.either_hears(hooks))
}

/// A model in the fight that overrides a hook, as a panic names it.
#[derive(Debug)]
#[expect(dead_code, reason = "read through `Debug`, in the panic that names it")]
pub(crate) enum Overrider<V: Rules> {
    Power(V::Power, CombatId),
    Monster(V::Monster),
    Relic(V::Relic),
    Potion(V::Potion),
    Card(sts2_core::CardIdent<V>),
    Modifier(V::Modifier),
}

/// Every model in the fight that overrides `hook`, for a panic to name. The
/// slow walk, which only a hook nobody has ported asks for.
#[cold]
pub(crate) fn listeners_to<V: Rules>(
    live: &Live<V>,
    belts: &Belts<V>,
    hook: ModelHook,
) -> Vec<Overrider<V>> {
    let hooks = Overrides::of(hook);
    let mut named = Vec::new();
    let Some(kinds) = Kinds::hearing(live, belts, hooks) else {
        return named;
    };
    listed(live, belts, hooks, kinds, |found| {
        named.push(match found {
            Found::Power { creature, held, .. } => Overrider::Power(held.power(), creature.id),
            Found::Monster { monster, .. } => Overrider::Monster(monster),
            Found::Relic { seat, slot, .. } => Overrider::Relic(belts[seat][slot].relic),
            Found::Potion { potion, .. } => Overrider::Potion(potion),
            Found::Card { seat, id, .. } => Overrider::Card(
                live.players[seat]
                    .cards
                    .named(id)
                    .expect("a card the fight lists as listening is in it")
                    .face
                    .ident,
            ),
            Found::Modifier { modifier, .. } => Overrider::Modifier(modifier.modifier),
        });
    });
    named
}

impl Kinds {
    /// The kinds that hear one of `hooks`, or nothing when none does.
    #[inline(always)]
    fn hearing<V: Rules>(live: &Live<V>, belts: &Belts<V>, hooks: Overrides) -> Option<Self> {
        let kinds = Self {
            powers: live.powers.hear(hooks),
            monsters: live.creatures.hear(hooks),
            belt: belts.any(|belt| belt.hear(hooks)),
            cards: live.players.any(|player| player.cards.hears(hooks)),
            others: live
                .players
                .any(|player| player.potions.either_hears(hooks)),
        };
        (kinds.powers || kinds.monsters || kinds.belt || kinds.cards || kinds.others)
            .then_some(kinds)
    }
}

/// Ask `ask` of every listener that overrides a hook in `hooks`, in the order
/// the game asks them, with every hook that listener overrides.
///
/// Nothing a listener answers here writes on the fight, so nothing is taken
/// before the first is asked. One walk serves every hook in `hooks`, which is
/// how a number's phases are asked together.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn each<V: Rules>(
    fight: Walk<'_, V>,
    hooks: Overrides,
    mut ask: impl FnMut(Heard<'_, V>, Overrides),
) {
    let Walk { live, belts } = fight;
    let Some(kinds) = Kinds::hearing(live, belts, hooks) else {
        return;
    };
    let onlookers = super::live::Onlookers {
        creatures: &live.creatures,
        powers: &live.powers,
        kept: &live.kept,
        belts: Some(belts),
        attack: &live.attack_results,
    };
    listed(
        live,
        belts,
        hooks,
        kinds,
        #[inline(always)]
        |found| match found {
            Found::Power {
                creature,
                held,
                overrides,
                ..
            } => ask(
                held.listener(creature.carrier(), live.history(), &onlookers),
                overrides,
            ),
            Found::Monster {
                creature,
                monster,
                overrides,
            } => ask(
                sts2_core::Listener::Monster {
                    monster,
                    creature: creature.id,
                },
                overrides,
            ),
            Found::Relic {
                seat,
                slot,
                overrides,
            } => ask(
                Heard::<V>::Relic {
                    state: &belts[seat][slot].state,
                    owner: seat.creature(),
                    fight: Some(&onlookers),
                    belt: belts[seat].relics(),
                },
                overrides,
            ),
            Found::Potion { potion, overrides } => {
                ask(sts2_core::Listener::Potion(potion), overrides)
            }
            Found::Card {
                seat,
                id,
                overrides,
            } => {
                let card = live.players[seat]
                    .cards
                    .named(id)
                    .expect("a card the fight lists as listening is in it");
                // What a card carries is asked only the hooks a walk raises, so
                // what the card hears here is its own.
                let ident = card.face.ident;
                if let Some(affliction) = ident.affliction
                    && sts2_core::AfflictionTable::overrides(affliction).intersects(hooks)
                {
                    unimplemented!("what {affliction:?} answers when asked {hooks:?}");
                }
                if let Some(enchanted) = ident.enchantment
                    && sts2_core::EnchantmentTable::overrides(enchanted.enchantment)
                        .intersects(hooks)
                {
                    unimplemented!("what {enchanted:?} answers when asked {hooks:?}");
                }
                let listener = sts2_core::Listener::Card {
                    card: ident.card,
                    id,
                    printed: &card,
                    pile: live.players[seat].cards.pile_of(id),
                    history: live.history(),
                };
                ask(listener, overrides);
            }
            Found::Modifier {
                modifier,
                overrides,
            } => ask(sts2_core::Listener::Modifier(modifier.modifier), overrides),
        },
    );
}

/// Whether any listener answers `hook` with `answer`.
///
/// Every `Hook.ShouldX` asks until one listener answers other than the
/// default, and that one listener settles it. So one is enough.
///
/// # Panics
///
/// A question the game asks of one power about itself, which is not raised.
#[inline(always)]
pub(crate) fn any_decides<V: Rules>(
    fight: Walk<'_, V>,
    hook: DecisionHook<V>,
    answer: bool,
) -> bool {
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one power, not raised"));
    let hooks = Overrides::of(model_hook);
    Kinds::hearing(fight.live, fight.belts, hooks).is_some()
        && decides::<V>(fight, hooks, hook, answer)
}

/// The first listener in the fight to answer `hook` with `answer`, by the
/// model it is. `Hook.ShouldPlay` names it as the preventer.
#[inline(always)]
pub(crate) fn first_deciding<V: Rules>(
    fight: Walk<'_, V>,
    hook: DecisionHook<V>,
    answer: bool,
) -> Option<sts2_core::ModelId<V>> {
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one power, not raised"));
    let hooks = Overrides::of(model_hook);
    Kinds::hearing(fight.live, fight.belts, hooks)?;
    first_of::<V>(fight, hooks, hook, answer)
}

/// [`first_deciding`] once something hears `hook`.
#[inline(never)]
fn first_of<V: Rules>(
    fight: Walk<'_, V>,
    hooks: Overrides,
    hook: DecisionHook<V>,
    answer: bool,
) -> Option<sts2_core::ModelId<V>> {
    let mut first = None;
    each::<V>(fight, hooks, |listener, _| {
        if first.is_none() && V::decide_hook(listener, hook).is_decision_and(|said| said == answer)
        {
            first = Some(match listener {
                sts2_core::Listener::Power { power, .. } => sts2_core::ModelId::Power(power),
                sts2_core::Listener::Monster { monster, .. } => {
                    sts2_core::ModelId::Monster(monster)
                }
                sts2_core::Listener::Card { card, .. } => sts2_core::ModelId::Card(card),
                sts2_core::Listener::Potion(potion) => sts2_core::ModelId::Potion(potion),
                sts2_core::Listener::Modifier(modifier) => sts2_core::ModelId::Modifier(modifier),
                // The relic whose model this is, which only its place on the
                // belt names.
                sts2_core::Listener::Relic { state, .. } => sts2_core::ModelId::Relic({
                    let (seat, slot) = super::relic::place_of::<V>(fight.belts, state);
                    fight.belts[seat][slot].relic
                }),
            });
        }
    });
    first
}

/// `hook` asked of the relic `relic` alone, the way the game asks the one
/// model a question named, and what it asked for.
///
/// # Panics
///
/// A relic that is not on the belt.
pub(crate) fn ask_relic<V: Rules>(
    live: &Live<V>,
    seat: sts2_core::SeatIndex,
    belts: &mut Belts<V>,
    relic: V::Relic,
    hook: EventHook<V>,
) -> sts2_core::HookEffects<V> {
    let slot = belts[seat]
        .iter()
        .position(|held| held.relic == relic)
        .expect("the relic asked is on the belt");
    let (state, belt) = belts[seat].state_beside(slot);
    let mut asked = sts2_core::HookEffects::new();
    V::relic_event_hook(state, belt, live.relic_owner(seat), hook, &mut asked);
    asked
}

/// [`any_decides`] once something hears `hook`.
#[inline(never)]
fn decides<V: Rules>(
    fight: Walk<'_, V>,
    hooks: Overrides,
    hook: DecisionHook<V>,
    answer: bool,
) -> bool {
    let mut decided = false;
    each::<V>(fight, hooks, |listener, _| {
        decided |= V::decide_hook(listener, hook).is_decision_and(|said| said == answer);
    });
    decided
}

/// `ReviveBeforeCombatEnd` for each player a won fight left dead:
/// `CreatureCmd.Heal` of one hit point. `HealInternal` brings the player
/// back with their hooks on again, and `AfterCurrentHpChanged` follows.
pub(crate) fn revive_the_dead<V: Rules>(fighting: super::Fighting<'_, V>) {
    let super::Fighting {
        live,
        streams,
        belts,
    } = fighting;
    let ascension = live.ascension;
    for seat in super::turn::seats(live) {
        if live.player(seat).alive {
            continue;
        }
        belts[seat].rehear();
        live.players[seat].potions.rehear();
        let mut board = super::board::Board::for_card(
            seat,
            live,
            streams.combat(),
            belts,
            ascension,
            None,
            None,
        );
        sts2_core::Fight::heal(&mut board, seat.creature(), num_traits::one());
    }
}

/// `AfterCombatEnd`, then `AfterCombatVictory`, raised on the fight's
/// listeners once the fight is won. The run asks the deck's cards first.
///
/// A Max HP gain at the first lands on the player's creature, and the run
/// carries out the rest.
///
/// A heal at the second lands on the player, whom a won fight leaves alive.
/// The run carries out a deck upgrade there.
///
/// # Panics
///
/// A listener asking for anything else at the second, which nothing here
/// carries out.
pub(crate) fn combat_ended<V: Rules>(live: &mut Live<V>, belts: &mut Belts<V>) {
    // What no one player owns acts as the first.
    let host = sts2_core::SeatIndex(0);
    // What a listener asks of the run, which carries it out as it leaves.
    let room = live.room();
    let mut walking = Walking::begin(live, belts, EventHook::CombatEnd { room });
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, live, belts))
    {
        // Each listener acts for its owner.
        let seat = owner::<V>(live, listener).unwrap_or(host);
        match effect {
            // `CreatureCmd.GainMaxHp` on the player's creature, which the
            // fight still holds: the maximum grows, and the health with it.
            HookEffect::GainMaxHp(amount) => {
                let gained = health::<V>(amount);
                let player = player_of(seat, live);
                player.max_hp = player.max_hp + gained;
                player.hp = player.hp + gained;
            }
            // `RelicCmd.Melt`, at once: the relic it melts hears no hook the
            // fight raises after this one.
            HookEffect::MeltFirstWax => belts[seat].melt_first_wax(),
            effect => live.leaving.push((seat, effect)),
        }
    }
    let early = |_: &mut Live<V>| EventHook::CombatVictoryEarly { room };
    let hook = early(live);
    let mut walking = Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, live, belts))
    {
        // Each listener acts for its owner.
        let seat = owner::<V>(live, listener).unwrap_or(host);
        match effect {
            // `CreatureCmd.Heal`, which caps at the maximum. The listeners
            // after it read the health it left.
            HookEffect::Heal(amount) => {
                let healed = health::<V>(amount);
                let player = player_of(seat, live);
                player.hp = (player.hp + healed).min(player.max_hp);
                let hook = early(live);
                if let Some(walking) = walking.as_mut() {
                    walking.rehook(hook);
                }
            }
            effect => unimplemented!("a listener asking for {effect:?} as a fight is won, early"),
        }
    }
    let mut walking = Walking::begin(live, belts, EventHook::CombatVictory { room });
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, live, belts))
    {
        // Each listener acts for its owner.
        let seat = owner::<V>(live, listener).unwrap_or(host);
        match effect {
            // `CreatureCmd.Heal`, which caps at the maximum.
            HookEffect::Heal(amount) => {
                let healed = health::<V>(amount);
                let player = player_of(seat, live);
                player.hp = (player.hp + healed).min(player.max_hp);
            }
            // The deck and the belt are the run's, which carries these out as
            // it leaves.
            effect @ (HookEffect::UpgradeRandomDeckCards { .. }
            | HookEffect::ReplaceRelic { .. }) => live.leaving.push((seat, effect)),
            effect => unimplemented!("a listener asking for {effect:?} as a fight is won"),
        }
    }
}

/// The player's creature, which a fight always holds.
fn player_of<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
) -> &mut super::live::Fighter<V> {
    let id = seat.creature();
    live.creatures
        .iter_mut()
        .find(|creature| creature.id == id)
        .expect("a fight has every player in it")
}

/// A game number as a creature's health.
fn health<V: Rules>(amount: V::Number) -> V::Health {
    num_traits::cast(amount).unwrap_or_else(|| unimplemented!("{amount:?}, which is not a health"))
}

/// `hook` asked of a run modifier inside a fight. What the modifier saves is
/// the run's, and no ported modifier writes it in a fight, so it is asked
/// over a fresh model.
///
/// # Panics
///
/// A modifier that writes its model in a fight.
fn ask_modifier<V: Rules>(
    modifier: sts2_core::RunModifier<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let fresh: V::ModifierState = sts2_core::ModifierModel::new(modifier.modifier);
    let mut state = fresh.clone();
    V::modifier_event_hook(modifier, &mut state, hook, asked);
    if state != fresh {
        unimplemented!("{modifier:?} writing its model in a fight over {hook:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hook is heard while any listener overriding it remains, and not once
    /// the last one leaves.
    #[test]
    fn a_hook_is_heard_while_a_listener_of_it_remains() {
        let both = Overrides::of(ModelHook::AfterDeath).with(ModelHook::AfterCardPlayed);
        let death = Overrides::of(ModelHook::AfterDeath);
        let mut counts = HookCounts::default();
        assert!(!counts.hears(death));

        counts.add(both);
        counts.add(death);
        counts.remove(both);
        assert!(counts.hears(death));
        assert!(!counts.hears(Overrides::of(ModelHook::AfterCardPlayed)));

        counts.remove(death);
        assert!(!counts.hears(death.with(ModelHook::AfterCardPlayed)));
    }
}
