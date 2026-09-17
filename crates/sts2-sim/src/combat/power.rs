//! The powers the creatures in a fight carry, as it asks them things.
//!
//! `Hook` walks every model in the fight and gives each the notification,
//! which for the powers is what this does. A power answers as its own model,
//! reading the stack its creature carries it as and the fields it saves.

use sts2_core::{EventHook, HookEffect, Rules};

use super::live::{Fighter, Live};

/// Deliver one notification to every power in the fight, and gather what they
/// ask for.
///
/// A power that writes on itself does so through the model the listener hands
/// it, so nothing about that comes back. Everything else comes back in
/// creature order, which is the order the game asks them in.
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
    let mut remaining = Vec::new();
    for effect in asked_of::<V>(live, Some(creature), hook) {
        match effect {
            HookEffect::RemoveFromOwner(power) => {
                if let Some(owner) = live.creatures.iter_mut().find(|owner| owner.id == creature) {
                    owner.powers.retain(|held| held.power != power);
                }
            }
            HookEffect::StunOwner(stunned) => {
                if let Some(owner) = live.creatures.iter_mut().find(|owner| owner.id == creature) {
                    owner.next_move = Some(stunned);
                }
                if let Some(machine) = live
                    .machines
                    .iter_mut()
                    .find(|held| held.creature == creature)
                {
                    machine.stunned = true;
                }
            }
            effect => remaining.push(effect),
        }
    }
    remaining
}

/// The body both of those share, over every creature or over one.
fn asked_of<V: Rules>(
    live: &mut Live<V>,
    only: Option<sts2_core::CombatId>,
    hook: EventHook<V>,
) -> Vec<HookEffect<V>> {
    let mut asked = Vec::new();
    let mut decremented: Vec<(sts2_core::CombatId, V::Power)> = Vec::new();

    for creature in live
        .creatures
        .iter_mut()
        .filter(|creature| only.is_none_or(|id| creature.id == id))
    {
        let owner = creature.side;
        let id = creature.id;
        for held in creature.powers.iter_mut() {
            let power = held.power;
            for effect in <V as sts2_core::Hooks>::event_hook(held.event_listener(owner), hook) {
                match effect {
                    HookEffect::Decrement => decremented.push((id, power)),
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
    asked
}

/// `BeforeSideTurnStart`, as the powers in the fight answer it.
///
/// Raised on every power, not only the ones on the side taking its turn. Which
/// side's turn opening a power answers is the power's own rule. Block gained
/// here lands on the creature the power sits on, which is why this walks the
/// creatures rather than hand the effect back.
pub(crate) fn before_turn_start<V: Rules>(
    creatures: &mut [Fighter<V>],
    side: V::CombatSide,
    turn: V::Count,
) {
    for creature in creatures {
        let owner = creature.side;
        let mut gained = <V::Number as num_traits::Zero>::zero();
        for held in creature.powers.iter_mut() {
            for effect in <V as sts2_core::Hooks>::event_hook(
                held.event_listener(owner),
                EventHook::BeforeSideTurnStart { side, turn },
            ) {
                match effect {
                    // `ValueProp.Unpowered`, so nothing the creature carries
                    // moves it.
                    HookEffect::GainBlock(amount) => gained = gained + amount,
                    other => unimplemented!("a power asking for {other:?} before a turn starts"),
                }
            }
        }
        creature.block = creature.block + gained;
    }
}

/// `BeforeSideTurnEndEarly`, including Plating before later end effects.
pub(crate) fn before_turn_end<V: Rules>(creatures: &mut [Fighter<V>], side: V::CombatSide) {
    for creature in creatures {
        let owner = creature.side;
        let mut gained = <V::Number as num_traits::Zero>::zero();
        for held in &mut creature.powers {
            for effect in <V as sts2_core::Hooks>::event_hook(
                held.event_listener(owner),
                EventHook::BeforeSideTurnEndEarly { side },
            ) {
                match effect {
                    HookEffect::GainBlock(amount) => gained = gained + amount,
                    other => unimplemented!("a power asking for {other:?} before a turn ends"),
                }
            }
        }
        creature.block = creature.block + gained;
    }
}
