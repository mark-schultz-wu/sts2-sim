//! The powers the creatures in a fight carry, as it asks them things.
//!
//! `Hook` walks every model in the fight and gives each the notification,
//! which for the powers is what this does. A power answers as its own model,
//! reading the stack its creature carries it as and the number it counts on
//! itself.

use sts2_core::{EventHook, HookEffect, Rules};

use super::live::{Counter, Live, counted};

/// Deliver one notification to every power in the fight, and gather what they
/// ask for.
///
/// A power that counts on itself is written here rather than handed back. The
/// effect names the instance that asked, and a fight command cannot carry
/// that. Everything else comes back in creature order, which is the order the
/// game asks them in.
pub(crate) fn notify<V: Rules>(live: &mut Live<V>, hook: EventHook<V>) -> Vec<HookEffect<V>> {
    asked_of::<V>(live, None, hook)
}

/// Deliver one notification to the powers on one creature.
///
/// `AfterDamageReceived` is raised on every listener in the fight, and each
/// reads whether the creature it names is its own. The powers on that creature
/// are the same set answering.
pub(crate) fn notify_creature<V: Rules>(
    live: &mut Live<V>,
    creature: sts2_core::CombatId,
    hook: EventHook<V>,
) -> Vec<HookEffect<V>> {
    asked_of::<V>(live, Some(creature), hook)
}

/// The body both of those share, over every creature or over one.
fn asked_of<V: Rules>(
    live: &mut Live<V>,
    only: Option<sts2_core::CombatId>,
    hook: EventHook<V>,
) -> Vec<HookEffect<V>> {
    let counters = live.counted.clone();
    let mut asked = Vec::new();
    let mut counted_now: Vec<Counter<V>> = Vec::new();
    let mut cleared: Vec<(sts2_core::CombatId, V::Power)> = Vec::new();

    let mut decremented: Vec<(sts2_core::CombatId, V::Power)> = Vec::new();

    for creature in live
        .creatures
        .iter()
        .filter(|creature| only.is_none_or(|id| creature.id == id))
    {
        for held in &creature.powers {
            let listener = held.listener(
                creature.side,
                counted::<V>(&counters, creature.id, held.power),
            );
            for effect in <V as sts2_core::Hooks>::event_hook(listener, hook) {
                match effect {
                    HookEffect::Count(amount) => counted_now.push(Counter {
                        creature: creature.id,
                        power: held.power,
                        amount,
                    }),
                    HookEffect::ClearCount => cleared.push((creature.id, held.power)),
                    HookEffect::Decrement => decremented.push((creature.id, held.power)),
                    effect => asked.push(effect),
                }
            }
        }
    }

    // `PowerCmd.Decrement`, which takes the instance off at zero the way a
    // duration tick does.
    let one = num_traits::one();
    for (id, power) in decremented {
        let Some(creature) = live.creatures.iter_mut().find(|creature| creature.id == id) else {
            continue;
        };
        for held in creature.powers.iter_mut() {
            if held.power == power
                && let sts2_core::PowerAmount::Counted(amount) = held.amount
            {
                held.amount = sts2_core::PowerAmount::Counted(amount - one);
            }
        }
        creature
            .powers
            .retain(|held| held.amount != sts2_core::PowerAmount::Counted(num_traits::zero()));
    }

    live.counted
        .retain(|held| !cleared.contains(&(held.creature, held.power)));
    for more in counted_now {
        match live
            .counted
            .iter_mut()
            .find(|held| held.creature == more.creature && held.power == more.power)
        {
            Some(held) => held.amount = held.amount + more.amount,
            None => live.counted.push(more),
        }
    }
    asked
}
