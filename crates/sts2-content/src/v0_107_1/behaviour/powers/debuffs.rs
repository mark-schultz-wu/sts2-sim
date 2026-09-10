//! The powers nothing wants.

use sts2_core::Holding;

use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;

/// `VulnerablePower`'s `DamageIncrease`.
const DAMAGE_INCREASE: f64 = 1.5;

/// `WeakPower`'s `DamageDecrease`.
const DAMAGE_DECREASE: f64 = 0.75;

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
        (power, _) => unimplemented!("what {power:?} does to a hit"),
    }
}

/// Both count down once the enemy side's turn ends. A debuff put on something
/// at the start of a round lasts that round out.
///
/// # Panics
///
/// A power nobody has ported.
pub(super) fn ticks_down(power: Power, side: CombatSide) -> bool {
    match power {
        Power::VulnerablePower | Power::WeakPower => side == CombatSide::Enemy,
        power => unimplemented!("when {power:?} counts down"),
    }
}
