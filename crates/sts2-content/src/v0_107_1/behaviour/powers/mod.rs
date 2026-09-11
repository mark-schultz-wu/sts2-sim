//! What a power does, ported.
//!
//! One function per hook, matching on the power the way `PowerModel`'s
//! subclasses override one. A power gets an arm when something in a fight can
//! apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

use sts2_core::Holding;

use super::super::generated::powers::Power;
use super::super::generated::runstate::CombatSide;

/// `VulnerablePower`'s `DamageIncrease`.
const DAMAGE_INCREASE: f64 = 1.5;

/// `WeakPower`'s `DamageDecrease`.
const DAMAGE_DECREASE: f64 = 0.75;

/// `ShrinkPower`'s own `DamageDecrease`, which the power stores as the
/// percentage taken off rather than as what is left.
const SHRUNK: f64 = 1.0 - 30.0 / 100.0;

/// What a power multiplies a hit by.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn damage_multiplier(power: Power, holding: Holding) -> f64 {
    match (power, holding) {
        // Vulnerable acts on the hits its owner takes.
        (Power::VulnerablePower, Holding::Target) => DAMAGE_INCREASE,
        // Weak acts on the hits its owner deals. So does Shrink, and only on a
        // powered attack. Every hit a fight deals here comes from a card or a
        // move, which is what powered means.
        (Power::WeakPower, Holding::Dealer) => DAMAGE_DECREASE,
        (Power::ShrinkPower, Holding::Dealer) => SHRUNK,
        (Power::VulnerablePower | Power::WeakPower | Power::ShrinkPower, _) => 1.0,
        // `EnergyNextTurnPower` has no `ModifyDamage`. It hands over an energy
        // and leaves.
        (Power::EnergyNextTurnPower, _) => 1.0,
        (power, _) => unimplemented!("what {power:?} does to a hit"),
    }
}

/// Whether a power counts down when this side's turn ends.
///
/// Weak and Vulnerable both count down once the enemy side's turn ends. A
/// debuff put on something at the start of a round lasts that round out.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn ticks_down(power: Power, side: CombatSide) -> bool {
    match power {
        Power::VulnerablePower | Power::WeakPower => side == CombatSide::Enemy,
        // `ShrinkPower.AfterSideTurnEnd` counts down on the side its owner is
        // on. Only an endless one is in a fight so far, and an endless power
        // has no number to count.
        Power::ShrinkPower => unimplemented!("when a counted Shrink runs out"),
        // It is spent rather than counted down. `AfterEnergyReset` gives the
        // whole amount over and removes the instance.
        Power::EnergyNextTurnPower => false,
        power => unimplemented!("when {power:?} counts down"),
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
        Power::VulnerablePower | Power::WeakPower | Power::ShrinkPower => false,
        power => unimplemented!("what {power:?} does as energy resets"),
    }
}
