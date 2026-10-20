//! The powers whose rules this build ports for itself, which the shared
//! powers hand over through [`crate::build::OwnRules`].

use sts2_core::{EventHook, Fixed, HookEffect, Modify, ModifyHook, PowerAmount};

use super::super::V0_107_1 as V;
use super::super::generated::powers::Power;
use crate::behaviour::powers::{Held, OUTBREAK_EVERY, State};
use crate::generated::runstate::CombatSide;

type Number = i32;

/// Whether `power`'s rule is this build's own.
pub(crate) fn own_power(power: Power) -> bool {
    matches!(power, Power::DiamondDiademPower | Power::OutbreakPower)
}

/// The state a power of this build's own starts with.
pub(crate) fn own_power_state(power: Power) -> Option<State<V>> {
    match power {
        Power::OutbreakPower => Some(State::Outbreak { power, poisoned: 0 }),
        _ => None,
    }
}

/// `ModifyHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_modify_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    _state: &State<V>,
    hook: ModifyHook<V>,
) -> Option<Modify<Number>> {
    let holder = held.holder;
    Some(match (power, amount, hook) {
        // `DiamondDiademPower.ModifyDamageMultiplicative`: half of a powered
        // hit on its holder.
        (
            Power::DiamondDiademPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(Fixed::new(5, 1)),
        (Power::DiamondDiademPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        _ => return None,
    })
}

/// `EventHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_event_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &mut State<V>,
    hook: EventHook<V>,
) -> Option<Option<HookEffect<V>>> {
    let holder = held.holder;
    Some(match (power, amount, hook) {
        // `OutbreakPower.AfterPowerAmountChanged`: each Poison its owner adds
        // counts, and every third deals its stack, unpowered, to each enemy
        // that can be hit.
        (
            Power::OutbreakPower,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: Power::PoisonPower,
                delta,
                applier: Some(applier),
                ..
            },
        ) if applier == holder && delta > 0 => {
            let State::Outbreak { poisoned, .. } = state else {
                unreachable!("Outbreak counts its poisonings")
            };
            *poisoned += 1;
            if *poisoned >= OUTBREAK_EVERY {
                *poisoned %= OUTBREAK_EVERY;
                Some(HookEffect::DamageOpponents(amount))
            } else {
                None
            }
        }
        (Power::OutbreakPower, PowerAmount::Endless, EventHook::PowerAmountChanged { .. }) => {
            unimplemented!("an endless Outbreak")
        }
        (Power::OutbreakPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `DiamondDiademPower.AfterSideTurnEnd`: gone once the enemies' turn
        // ends.
        (
            Power::DiamondDiademPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) => Some(HookEffect::Remove),
        (Power::DiamondDiademPower, _, EventHook::SideTurnEnd { .. }) => None,
        _ => return None,
    })
}
