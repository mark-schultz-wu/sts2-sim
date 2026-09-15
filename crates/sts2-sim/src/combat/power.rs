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
    let counters = live.counted.clone();
    let mut asked = Vec::new();
    let mut counted_now: Vec<Counter<V>> = Vec::new();
    let mut cleared: Vec<(sts2_core::CombatId, V::Power)> = Vec::new();

    for creature in &live.creatures {
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
                    effect => asked.push(effect),
                }
            }
        }
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
