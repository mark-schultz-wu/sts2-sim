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
//! The walk is taken before anyone is asked, which is what the game does. So a
//! listener added while it runs is not asked, and one removed is skipped.

use smallvec::SmallVec;
use sts2_core::{
    CardId, CombatId, DecisionHook, EventHook, HookEffect, ModelHook, ModelId, MonsterTable as _,
    PowerTable as _, RelicTable as _, Rules,
};

use super::live::{Heard, HeardEvent, Live};
use crate::run::HeldRelic;

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

/// What a walk reads without writing: the fight, and the belt its player
/// carries.
pub(crate) struct Walk<'a, V: Rules> {
    pub(crate) live: &'a Live<V>,
    pub(crate) relics: &'a [HeldRelic<V>],
}

impl<V: Rules> Clone for Walk<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Walk<'_, V> {}

/// Whether any listener in the fight overrides `hook`.
///
/// The walk without the list, which is the answer for most events. Nothing is
/// built to find out that nobody listens.
fn heard<V: Rules>(live: &Live<V>, relics: &[HeldRelic<V>], hook: ModelHook) -> bool {
    live.creatures.iter().any(|creature| {
        creature
            .powers
            .iter()
            .any(|held| held.power().overrides().contains(hook))
            || creature
                .carrier()
                .monster
                .is_some_and(|monster| monster.overrides().contains(hook))
    }) || relics
        .iter()
        .any(|held| held.relic.overrides().contains(hook))
        || live.cards.hears(hook)
}

/// Every listener that overrides `hook`, in the order the game asks them.
pub(crate) fn listeners<V: Rules>(
    live: &Live<V>,
    relics: &[HeldRelic<V>],
    hook: ModelHook,
) -> SmallVec<[Listener<V>; 8]> {
    let mut listening = SmallVec::new();
    for creature in live.creatures.iter() {
        for (at, held) in creature.powers.iter().enumerate() {
            if held.power().overrides().contains(hook) {
                listening.push(Listener::Power {
                    owner: creature.id,
                    at: at as u8,
                    power: held.power(),
                });
            }
        }
        if let Some(monster) = creature.carrier().monster
            && monster.overrides().contains(hook)
        {
            listening.push(Listener::Monster {
                creature: creature.id,
                monster,
            });
        }
        if let Some(ModelId::Character(_)) = creature.model {
            for (slot, held) in relics.iter().enumerate() {
                if held.relic.overrides().contains(hook) {
                    listening.push(Listener::Relic { slot: slot as u8 });
                }
            }
            listening.extend(live.cards.listening(hook).map(|id| Listener::Card { id }));
        }
    }
    listening
}

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
pub(crate) fn notify<V: Rules>(
    live: &mut Live<V>,
    relics: &mut [HeldRelic<V>],
    hook: EventHook<V>,
) -> SmallVec<[(Listener<V>, HookEffect<V>); 4]> {
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one model, not raised"));
    let mut asked = SmallVec::new();
    if !heard::<V>(live, relics, model_hook) {
        return asked;
    }
    let listening = listeners::<V>(live, relics, model_hook);
    let mut answer = sts2_core::HookEffects::new();
    let mut decremented: SmallVec<[Listener<V>; 4]> = SmallVec::new();
    for listener in listening {
        match listener {
            Listener::Power { owner, at, power } => {
                let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) else {
                    continue;
                };
                let carrier = creature.carrier();
                let Some(held) = creature
                    .powers
                    .get_mut(usize::from(at))
                    .filter(|held| held.power() == power)
                else {
                    continue;
                };
                V::event_hook(held.event_listener(carrier), hook, &mut answer);
            }
            Listener::Relic { slot } => {
                let held = &mut relics[usize::from(slot)];
                V::event_hook(HeardEvent::<V>::Relic(&mut held.state), hook, &mut answer);
            }
            Listener::Monster { creature, monster } => {
                if !live.creatures.iter().any(|c| c.id == creature) {
                    continue;
                }
                V::event_hook(
                    HeardEvent::<V>::Monster { monster, creature },
                    hook,
                    &mut answer,
                );
            }
            Listener::Card { id } => {
                let Some(card) = live.cards.named(id) else {
                    continue;
                };
                V::event_hook(
                    sts2_core::EventListener::Card {
                        card: card.face.ident.card,
                        id,
                        printed: &card,
                    },
                    hook,
                    &mut answer,
                );
            }
        }
        for effect in answer.drain(..) {
            match (listener, effect) {
                (Listener::Power { .. }, HookEffect::Decrement) => decremented.push(listener),
                (listener, effect) => asked.push((listener, effect)),
            }
        }
    }

    // `PowerCmd.Decrement`, which takes the instance off at zero the way a
    // duration tick does.
    for listener in decremented {
        let Listener::Power { owner, power, .. } = listener else {
            continue;
        };
        let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) else {
            continue;
        };
        for held in creature.powers.iter_mut() {
            if held.power() == power && held.amount.count().is_some() {
                held.amount = held.amount.ticked();
            }
        }
        creature.powers.retain(|held| !held.amount.spent());
    }
    asked
}

/// Ask `ask` of every listener that overrides `hook`, in the order the game
/// asks them.
///
/// Nothing a listener answers here writes on the fight, so nothing is taken
/// before the first is asked.
pub(crate) fn each<V: Rules>(
    fight: Walk<'_, V>,
    hook: ModelHook,
    mut ask: impl FnMut(Heard<'_, V>),
) {
    let Walk { live, relics } = fight;
    if !heard::<V>(live, relics, hook) {
        return;
    }
    for creature in live.creatures.iter() {
        for held in creature.powers.iter() {
            if held.power().overrides().contains(hook) {
                ask(held.listener(creature.carrier()));
            }
        }
        if let Some(monster) = creature.carrier().monster
            && monster.overrides().contains(hook)
        {
            ask(sts2_core::Listener::Monster {
                monster,
                creature: creature.id,
            });
        }
        if let Some(ModelId::Character(_)) = creature.model {
            for held in relics {
                if held.relic.overrides().contains(hook) {
                    ask(Heard::<V>::Relic(&held.state));
                }
            }
            for id in live.cards.listening(hook) {
                let card = live
                    .cards
                    .named(id)
                    .expect("a card the fight lists as listening is in it");
                ask(sts2_core::Listener::Card {
                    card: card.face.ident.card,
                    id,
                    printed: &card,
                });
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
pub(crate) fn any_decides<V: Rules>(
    fight: Walk<'_, V>,
    hook: DecisionHook<V>,
    answer: bool,
) -> bool {
    let model_hook = hook
        .model_hook()
        .unwrap_or_else(|| unreachable!("{hook:?} is asked of one power, not raised"));
    let mut decided = false;
    each::<V>(fight, model_hook, |listener| {
        decided |= V::decide_hook(listener, hook).is_decision_and(|said| said == answer);
    });
    decided
}

/// `AfterCombatEnd`, raised on every listener once the fight is won.
///
/// The listeners that counted something in the fight forget it here. The
/// belt a run carries out of a fight is then the belt the game's screens
/// show.
///
/// # Panics
///
/// A listener asking for something as the fight ends, which nothing here
/// carries out.
pub(crate) fn combat_ended<V: Rules>(live: &mut Live<V>, relics: &mut [HeldRelic<V>]) {
    if let Some((_, effect)) = notify::<V>(live, relics, EventHook::CombatEnd).first() {
        unimplemented!("a listener asking for {effect:?} as a fight ends")
    }
}
