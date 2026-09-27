//! Every listener in a fight, asked in the order the game asks them.
//!
//! `Hook.AfterX` walks `CombatState.IterateHookListeners`: each creature in
//! turn, allies before enemies, its powers first. A monster's creature then
//! brings the monster's own model, and the player's its relics and every card
//! in its five piles. A listener decides for
//! itself whether an event concerns it, from what the event carries.
//!
//! A listener whose class does not override a hook inherits
//! `AbstractModel`'s, which does nothing. So the walk skips it, and only the
//! listeners that override a hook are ever asked it.
//!
//! The game takes the list before anyone is asked. Nothing a listener answers
//! here adds a listener or takes one away, because every effect is carried out
//! once all have answered. So a walk asks each listener where it stands.

use smallvec::SmallVec;
use sts2_core::{
    CardId, CardTable as _, CombatId, DecisionHook, EventHook, HookEffect, ModelHook, ModelId,
    MonsterTable as _, Overrides, PowerTable as _, RelicTable as _, Rules,
};

use super::live::{Heard, Live};
use super::powers::Powers as _;
use crate::run::Belt;

/// What the listeners a hook reached asked for, in the order they asked, each
/// beside the listener that asked it.
pub(crate) type Answers<V> = SmallVec<[(Listener<V>, HookEffect<V>); 4]>;

/// One listener, by what names it rather than by a borrow of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Listener<V: Rules> {
    /// The power at `at` on the creature `owner`.
    Power {
        owner: CombatId,
        at: u8,
        power: V::Power,
    },
    /// The relic in belt slot `slot`.
    Relic {
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
        for at in bits(hooks) {
            self.counts[at] += 1;
        }
        self.heard = self.heard.union(hooks);
    }

    /// A listener that overrides `hooks` leaves.
    pub(crate) fn remove(&mut self, hooks: Overrides) {
        for at in bits(hooks) {
            self.counts[at] -= 1;
            if self.counts[at] == 0 {
                self.heard = Overrides::from_bits(self.heard.bits() & !(1 << at));
            }
        }
    }

    /// Whether any listener overrides a hook in `hooks`.
    pub(crate) fn hears(&self, hooks: Overrides) -> bool {
        self.heard.intersects(hooks)
    }
}

/// Each hook in `hooks`, by its place in [`ModelHook`], and nothing for a
/// hook the set does not hold.
fn bits(hooks: Overrides) -> impl Iterator<Item = usize> {
    let mut left = hooks.bits();
    std::iter::from_fn(move || {
        (left != 0).then(|| {
            let at = left.trailing_zeros() as usize;
            left &= left - 1;
            at
        })
    })
}

/// What a walk reads without writing: the fight, and the belt its player
/// carries.
pub(crate) struct Walk<'a, V: Rules> {
    pub(crate) live: &'a Live<V>,
    pub(crate) relics: &'a Belt<V>,
}

impl<V: Rules> Clone for Walk<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Walk<'_, V> {}

/// Raise `hook` on every listener in the fight, and gather what they ask for,
/// in the order they asked.
///
/// A power's `Decrement` is its own business, and is carried out here once
/// every listener has answered. Everything else is the caller's.
///
/// # Panics
///
/// A hook that is not one `AbstractModel` declares, which is asked of one
/// model directly rather than raised.
#[tracing::instrument(level = "trace", skip_all, fields(hook = hook.model_hook().map_or("", ModelHook::name)))]
#[inline(always)]
pub(crate) fn notify<V: Rules>(
    live: &mut Live<V>,
    relics: &mut Belt<V>,
    hook: EventHook<V>,
) -> Answers<V> {
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one model, not raised"));
    match Kinds::hearing(live, relics, Overrides::of(model_hook)) {
        Some(kinds) => raise(live, relics, hook, model_hook, kinds),
        None => SmallVec::new(),
    }
}

/// Which kinds of listener hear a hook, and so which a walk looks through.
#[derive(Clone, Copy)]
struct Kinds {
    powers: bool,
    monsters: bool,
    belt: bool,
    cards: bool,
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
    let Some(kinds) = Kinds::hearing(fight.live, fight.relics, hooks) else {
        return target;
    };
    if kinds.monsters || kinds.belt || kinds.cards {
        unimplemented!("a listener other than a power choosing where a hit lands");
    }
    let mut chosen = target;
    for creature in fight.live.creatures.iter() {
        for held in fight.live.powers.of(creature.id) {
            if !held.power().overrides().intersects(hooks) {
                continue;
            }
            let hook = DecisionHook::TakesUnblockedDamage {
                target: chosen,
                powered,
                pet_of: creature.owner,
                alive: creature.alive,
            };
            let onlookers = super::live::Onlookers {
                creatures: &fight.live.creatures,
                powers: &fight.live.powers,
                relics: fight.relics,
            };
            let listener = held.listener(creature.carrier(), fight.live.history(), &onlookers);
            if V::decide_hook(listener, hook).is_decision_and(|takes| takes) {
                chosen = creature.id;
            }
        }
    }
    chosen
}

/// Whether anything in the fight overrides `hook`.
///
/// For a hook the fight raises nowhere yet. A listener that answers one is a
/// path nobody has ported, and asking this is how it stops by name.
pub(crate) fn heard<V: Rules>(live: &Live<V>, relics: &Belt<V>, hook: ModelHook) -> bool {
    Kinds::hearing(live, relics, Overrides::of(hook)).is_some()
}

/// A model in the fight that overrides a hook, as a panic names it.
#[derive(Debug)]
#[expect(dead_code, reason = "read through `Debug`, in the panic that names it")]
pub(crate) enum Overrider<V: Rules> {
    Power(V::Power, CombatId),
    Monster(V::Monster),
    Relic(V::Relic),
    Card(sts2_core::CardIdent<V>),
}

/// Every model in the fight that overrides `hook`, for a panic to name. The
/// slow walk, which only a hook nobody has ported asks for.
#[cold]
pub(crate) fn listeners_to<V: Rules>(
    live: &Live<V>,
    relics: &Belt<V>,
    hook: ModelHook,
) -> Vec<Overrider<V>> {
    let hooks = Overrides::of(hook);
    let mut named = Vec::new();
    for creature in live.creatures.iter() {
        for held in live.powers.of(creature.id) {
            if held.power().overrides().intersects(hooks) {
                named.push(Overrider::Power(held.power(), creature.id));
            }
        }
        if let Some(monster) = creature.carrier().monster
            && monster.overrides().intersects(hooks)
        {
            named.push(Overrider::Monster(monster));
        }
    }
    for held in relics.iter() {
        if held.relic.overrides().intersects(hooks) {
            named.push(Overrider::Relic(held.relic));
        }
    }
    for (id, _) in live.cards.listening(hooks) {
        if let Some(card) = live.cards.named(id) {
            named.push(Overrider::Card(card.face.ident));
        }
    }
    named
}

impl Kinds {
    /// The kinds that hear one of `hooks`, or nothing when none does.
    fn hearing<V: Rules>(live: &Live<V>, relics: &Belt<V>, hooks: Overrides) -> Option<Self> {
        let kinds = Self {
            powers: live.powers.hear(hooks),
            monsters: live.creatures.hear(hooks),
            belt: relics.hear(hooks),
            cards: live.cards.hears(hooks),
        };
        (kinds.powers || kinds.monsters || kinds.belt || kinds.cards).then_some(kinds)
    }
}

/// [`notify`] once something hears `hook`.
#[inline(never)]
fn raise<V: Rules>(
    live: &mut Live<V>,
    relics: &mut Belt<V>,
    hook: EventHook<V>,
    model_hook: ModelHook,
    kinds: Kinds,
) -> SmallVec<[(Listener<V>, HookEffect<V>); 4]> {
    let Kinds {
        powers,
        monsters,
        belt,
        cards,
    } = kinds;
    let hooks = Overrides::of(model_hook);
    let mut answer = sts2_core::HookEffects::new();
    // Returned as it is, so it is built where the caller receives it.
    let mut asked = SmallVec::new();
    let mut decremented: SmallVec<[Listener<V>; 4]> = SmallVec::new();
    for creature in 0..live.creatures.len() {
        let (id, carrier, model) = {
            let fighter = &live.creatures[creature];
            (fighter.id, fighter.carrier(), fighter.model)
        };
        if powers {
            for (at, held) in live.powers.of_mut(id).enumerate() {
                let power = held.power();
                if power.overrides().contains(model_hook) {
                    V::power_event_hook(
                        held.event_power(carrier, live.history.view()),
                        hook,
                        &mut answer,
                    );
                    let listener = Listener::Power {
                        owner: id,
                        at: at as u8,
                        power,
                    };
                    take(&mut asked, &mut decremented, listener, &mut answer);
                }
            }
        }
        if monsters
            && let Some(monster) = carrier.monster
            && monster.overrides().contains(model_hook)
        {
            V::monster_event_hook(monster, id, hook, &mut answer);
            let listener = Listener::Monster {
                creature: id,
                monster,
            };
            take(&mut asked, &mut decremented, listener, &mut answer);
        }
        let Some(ModelId::Character(_)) = model else {
            continue;
        };
        if belt {
            // The listening slots alone, in slot order.
            let mut left = relics.hearing(hooks);
            while left != 0 {
                let slot = left.trailing_zeros() as usize;
                left &= left - 1;
                let held = &mut relics[slot];
                V::relic_event_hook(&mut held.state, hook, &mut answer);
                take(
                    &mut asked,
                    &mut decremented,
                    Listener::Relic { slot: slot as u8 },
                    &mut answer,
                );
            }
        }
        if cards {
            for (id, _) in live.cards.listening(hooks) {
                let card = live
                    .cards
                    .named(id)
                    .expect("a card the fight lists as listening is in it");
                // The card's own override, then its enchantment's, which is
                // the order the combat state lists them in.
                if card.face.ident.card.overrides().contains(model_hook) {
                    V::card_event_hook(card.face.ident.card, id, &card, hook, &mut answer);
                }
                if let Some(enchanted) = card.face.ident.enchantment
                    && <V::Enchantment as sts2_core::EnchantmentTable>::overrides(
                        enchanted.enchantment,
                    )
                    .contains(model_hook)
                {
                    V::enchantment_event_hook(enchanted, id, hook, &mut answer);
                }
                take(
                    &mut asked,
                    &mut decremented,
                    Listener::Card { id },
                    &mut answer,
                );
            }
        }
    }

    // `PowerCmd.Decrement`, which takes the instance off at zero the way a
    // duration tick does.
    for &listener in &decremented {
        let Listener::Power { owner, power, .. } = listener else {
            continue;
        };
        for held in live.powers.of_mut(owner) {
            if held.power() == power && held.amount.count().is_some() {
                held.amount = held.amount.ticked();
            }
        }
        live.powers.retain(owner, |held| !held.amount.spent());
    }
    asked
}

/// Take what `listener` answered, which leaves `answer` empty for the next.
/// A power's `Decrement` goes to `decremented`, and every other effect goes to
/// `asked` beside the listener that asked for it. Most listeners answer
/// nothing.
#[inline(always)]
fn take<V: Rules>(
    asked: &mut SmallVec<[(Listener<V>, HookEffect<V>); 4]>,
    decremented: &mut SmallVec<[Listener<V>; 4]>,
    listener: Listener<V>,
    answer: &mut sts2_core::HookEffects<V>,
) {
    if answer.is_empty() {
        return;
    }
    for &effect in answer.iter() {
        match (listener, effect) {
            (Listener::Power { .. }, HookEffect::Decrement) => decremented.push(listener),
            (listener, effect) => asked.push((listener, effect)),
        }
    }
    answer.clear();
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
    let Walk { live, relics } = fight;
    let Some(Kinds {
        powers,
        monsters,
        belt,
        cards,
    }) = Kinds::hearing(live, relics, hooks)
    else {
        return;
    };
    let onlookers = super::live::Onlookers {
        creatures: &live.creatures,
        powers: &live.powers,
        relics,
    };
    for creature in live.creatures.iter() {
        if powers {
            for held in live.powers.of(creature.id) {
                let overrides = held.power().overrides();
                if overrides.intersects(hooks) {
                    ask(
                        held.listener(creature.carrier(), live.history(), &onlookers),
                        overrides,
                    );
                }
            }
        }
        if monsters
            && let Some(monster) = creature.carrier().monster
            && monster.overrides().intersects(hooks)
        {
            let listener = sts2_core::Listener::Monster {
                monster,
                creature: creature.id,
            };
            ask(listener, monster.overrides());
        }
        let Some(ModelId::Character(_)) = creature.model else {
            continue;
        };
        if belt {
            // The listening slots alone, in slot order.
            let mut left = relics.hearing(hooks);
            while left != 0 {
                let held = &relics[left.trailing_zeros() as usize];
                left &= left - 1;
                ask(Heard::<V>::Relic(&held.state), held.relic.overrides());
            }
        }
        if cards {
            for (id, overrides) in live.cards.listening(hooks) {
                let card = live
                    .cards
                    .named(id)
                    .expect("a card the fight lists as listening is in it");
                // An enchantment is asked only the hooks `raise` raises, so
                // what the card hears here is its own.
                if let Some(enchanted) = card.face.ident.enchantment
                    && <V::Enchantment as sts2_core::EnchantmentTable>::overrides(
                        enchanted.enchantment,
                    )
                    .intersects(hooks)
                {
                    unimplemented!("what {enchanted:?} answers when asked {hooks:?}");
                }
                let listener = sts2_core::Listener::Card {
                    card: card.face.ident.card,
                    id,
                    printed: &card,
                };
                ask(listener, overrides);
            }
        }
    }
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
    Kinds::hearing(fight.live, fight.relics, hooks).is_some()
        && decides::<V>(fight, hooks, hook, answer)
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

/// `AfterCombatEnd`, then `AfterCombatVictory`, raised on every listener
/// once the fight is won.
///
/// The listeners that counted something in the fight forget it at the first.
/// A Max HP gain there lands on the player's creature. The fight keeps the
/// rest of what it asks for the run to carry out.
///
/// A heal at the second lands on the player, whom a won fight leaves alive.
///
/// # Panics
///
/// A listener asking for anything else at the second, which nothing here
/// carries out.
pub(crate) fn combat_ended<V: Rules>(live: &mut Live<V>, relics: &mut Belt<V>) {
    // What a listener asks of the run, which carries it out as it leaves.
    let room = live.room();
    for (_, effect) in notify::<V>(live, relics, EventHook::CombatEnd { room }) {
        match effect {
            // `CreatureCmd.GainMaxHp` on the player's creature, which the
            // fight still holds: the maximum grows, and the health with it.
            HookEffect::GainMaxHp(amount) => {
                let gained = health::<V>(amount);
                let player = player_of(live);
                player.max_hp = player.max_hp + gained;
                player.hp = player.hp + gained;
            }
            effect => live.leaving.push(effect),
        }
    }
    for (_, effect) in notify::<V>(live, relics, EventHook::CombatVictory { room }) {
        match effect {
            // `CreatureCmd.Heal`, which caps at the maximum.
            HookEffect::Heal(amount) => {
                let healed = health::<V>(amount);
                let player = player_of(live);
                player.hp = (player.hp + healed).min(player.max_hp);
            }
            effect => unimplemented!("a listener asking for {effect:?} as a fight is won"),
        }
    }
}

/// The player's creature, which a fight always holds.
fn player_of<V: Rules>(live: &mut Live<V>) -> &mut super::live::Fighter<V> {
    live.creatures
        .iter_mut()
        .find(|creature| creature.side == <V::CombatSide as sts2_core::SideKinds>::PLAYER)
        .expect("a fight has the player in it")
}

/// A game number as a creature's health.
fn health<V: Rules>(amount: V::Number) -> V::Health {
    num_traits::cast(amount).unwrap_or_else(|| unimplemented!("{amount:?}, which is not a health"))
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
