//! The powers nothing wants.

use sts2_core::Holding;

use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;

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
pub(super) fn damage_multiplier(power: Power, holding: Holding) -> f64 {
    match (power, holding) {
        // Vulnerable acts on the hits its owner takes.
        (Power::VulnerablePower, Holding::Target) => DAMAGE_INCREASE,
        (Power::VulnerablePower, Holding::Dealer) => 1.0,
        // Weak acts on the hits its owner deals.
        (Power::WeakPower, Holding::Dealer) => DAMAGE_DECREASE,
        (Power::WeakPower, Holding::Target) => 1.0,
        // So does Shrink, and only on a powered attack. Every hit a fight
        // deals here comes from a card or a move, which is what powered means.
        (Power::ShrinkPower, Holding::Dealer) => SHRUNK,
        (Power::ShrinkPower, Holding::Target) => 1.0,
        (power, _) => unimplemented!("what {power:?} does to a hit"),
    }
}

/// Both count down once the enemy side's turn ends. A debuff put on something
/// at the start of a round lasts that round out.
///
/// # Panics
///
/// A power nobody has ported.
pub(super) fn ticks_down(power: Power, amount: i32, side: CombatSide) -> bool {
    match power {
        Power::VulnerablePower | Power::WeakPower => side == CombatSide::Enemy,
        // A Shrink the game stores as a negative number is `IsInfinite`, and
        // an infinite one never counts down.
        Power::ShrinkPower if amount < 0 => false,
        Power::ShrinkPower => unimplemented!("when a counted Shrink runs out"),
        power => unimplemented!("when {power:?} counts down"),
    }
}
