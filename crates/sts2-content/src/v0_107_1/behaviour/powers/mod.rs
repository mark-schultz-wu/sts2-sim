//! What a power does, ported.
//!
//! One function per hook, matching on the power the way `PowerModel`'s
//! subclasses override one. A power gets an arm when something in a fight can
//! apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

use sts2_core::{Holding, Modify, PowerAmount, SideTurnEnd};

use super::super::V0_107_1 as V;
use super::super::generated::powers::Power;
use super::super::generated::runstate::CombatSide;

/// `VulnerablePower`'s `DamageIncrease`.
const DAMAGE_INCREASE: f64 = 1.5;

/// `WeakPower`'s `DamageDecrease`.
const DAMAGE_DECREASE: f64 = 0.75;

/// `ShrinkPower`'s own `DamageDecrease`, which the power stores as the
/// percentage taken off rather than as what is left.
const SHRUNK: f64 = 1.0 - 30.0 / 100.0;

/// What a power does to a hit, which is its `Hook.ModifyDamage`.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn damage_hook(power: Power, holding: Holding, amount: PowerAmount<i32>) -> Modify<i32> {
    match (power, holding) {
        // `StrengthPower.ModifyDamageAdditive` adds its whole amount to the
        // hits its owner deals, and does nothing to the ones it takes.
        (Power::StrengthPower, Holding::Dealer) => match amount {
            PowerAmount::Counted(amount) => Modify::Added(amount),
            PowerAmount::Endless => unimplemented!("an endless Strength"),
        },
        (Power::StrengthPower, Holding::Target) => Modify::Nothing,
        // Vulnerable acts on the hits its owner takes.
        (Power::VulnerablePower, Holding::Target) => Modify::Scaled(DAMAGE_INCREASE),
        // Weak acts on the hits its owner deals. So does Shrink, and only on a
        // powered attack. Every hit a fight deals here comes from a card or a
        // move, which is what powered means.
        (Power::WeakPower, Holding::Dealer) => Modify::Scaled(DAMAGE_DECREASE),
        (Power::ShrinkPower, Holding::Dealer) => Modify::Scaled(SHRUNK),
        (Power::VulnerablePower | Power::WeakPower | Power::ShrinkPower, _) => Modify::Nothing,
        // `EnergyNextTurnPower` has no `ModifyDamage`. It hands over an energy
        // at the reset and goes.
        (
            Power::BlockNextTurnPower
            | Power::EnergyNextTurnPower
            | Power::IllusionPower
            | Power::MinionPower,
            _,
        ) => Modify::Nothing,
        (power, _) => unimplemented!("what {power:?} does to a hit"),
    }
}

/// What a power does to a block gain, which is its `Hook.ModifyBlock`.
///
/// None of them overrides it. The debuffs and Strength are an attack's
/// numbers, and `EnergyNextTurnPower` touches the energy pool.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn block_hook(power: Power, _amount: PowerAmount<i32>) -> Modify<i32> {
    match power {
        Power::VulnerablePower
        | Power::WeakPower
        | Power::ShrinkPower
        | Power::BlockNextTurnPower
        | Power::EnergyNextTurnPower
        | Power::StrengthPower
        | Power::IllusionPower
        | Power::MinionPower => Modify::Nothing,
        power => unimplemented!("what {power:?} does to a block gain"),
    }
}

/// What a power does when this side's turn ends, which is its
/// `AfterSideTurnEnd`.
///
/// Weak and Vulnerable tick a duration down, and only at the enemy's end. So a
/// debuff put on something at the start of a round lasts that round out.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn side_turn_end(power: Power, side: CombatSide) -> SideTurnEnd {
    match (power, side) {
        (Power::VulnerablePower | Power::WeakPower, CombatSide::Enemy) => {
            SideTurnEnd::TicksDownDuration
        }
        (Power::VulnerablePower | Power::WeakPower, _) => SideTurnEnd::Nothing,
        // `ShrinkPower.AfterSideTurnEnd` decrements and removes, at either
        // side's end. It is a different command from a duration tick, and only
        // an endless Shrink is in a fight so far.
        (Power::ShrinkPower, _) => unimplemented!("what a counted Shrink does when a turn ends"),
        // Neither has an `AfterSideTurnEnd`. `EnergyNextTurnPower` is spent at
        // the reset instead, which is a hook of its own, and Strength lasts
        // the fight.
        (
            Power::EnergyNextTurnPower
            | Power::BlockNextTurnPower
            | Power::StrengthPower
            | Power::IllusionPower
            | Power::MinionPower,
            _,
        ) => SideTurnEnd::Nothing,
        (power, _) => unimplemented!("what {power:?} does when a turn ends"),
    }
}

/// Whether a power hands its amount to the player as energy resets, and then
/// goes.
///
/// `PowerModel.AfterEnergyReset`, of which one implementation gives energy.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn spent_on_energy_reset(power: Power) -> bool {
    match power {
        Power::EnergyNextTurnPower => true,
        Power::VulnerablePower
        | Power::WeakPower
        | Power::ShrinkPower
        | Power::BlockNextTurnPower
        | Power::StrengthPower
        | Power::IllusionPower
        | Power::MinionPower => false,
        power => unimplemented!("what {power:?} does as energy resets"),
    }
}

/// The move Illusion installs instead of letting its owner leave the fight.
pub(crate) fn revival_move(power: Power) -> Option<sts2_core::NextMove<V>> {
    use super::super::generated::monsters::{IntentKind, MoveId};

    (power == Power::IllusionPower).then(|| sts2_core::NextMove {
        id: MoveId::ReviveMove,
        intents: [sts2_core::Intent {
            kind: IntentKind::Heal,
            attack: None,
        }]
        .into_iter()
        .collect(),
    })
}

/// Whether a power lets its dead owner be removed from the combat roster.
pub(crate) fn remove_creature_after_death(power: Power) -> Option<bool> {
    (power == Power::IllusionPower).then_some(false)
}

/// Dispatch one of the event hooks the simulator currently supports.
pub(crate) fn event_hook(
    power: Power,
    hook: sts2_core::EventHook<V>,
) -> Vec<sts2_core::HookEffect<V>> {
    use sts2_core::{EventHook, HookEffect};

    let effect = match hook {
        EventHook::SideTurnEnd { side, .. }
            if side_turn_end(power, side) == SideTurnEnd::TicksDownDuration =>
        {
            Some(HookEffect::TickDownDuration)
        }
        EventHook::EnergyReset { amount } if spent_on_energy_reset(power) => {
            let PowerAmount::Counted(amount) = amount else {
                unimplemented!("an endless {power:?} handing over energy")
            };
            Some(HookEffect::GainEnergyAndRemove(amount))
        }
        EventHook::BlockCleared {
            amount: PowerAmount::Counted(amount),
        } if power == Power::BlockNextTurnPower => Some(HookEffect::GainBlockAndRemove(amount)),
        EventHook::AfterDeath => revival_move(power).map(HookEffect::InstallMove),
        _ => None,
    };
    effect.into_iter().collect()
}
