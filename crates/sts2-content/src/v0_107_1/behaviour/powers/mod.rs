//! What a power does, ported.
//!
//! One module per kind, the way `generated/powers/` is filed. A power gets a
//! line here when something in a fight can apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

use sts2_core::Holding;

use super::super::generated::powers::Power;
use super::super::generated::runstate::CombatSide;

mod debuffs;

/// What a power multiplies a hit by.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn damage_multiplier(power: Power, holding: Holding) -> f64 {
    debuffs::damage_multiplier(power, holding)
}

/// Whether a power counts down when this side's turn ends.
///
/// # Panics
///
/// A power nobody has ported.
pub(crate) fn ticks_down(power: Power, amount: i32, side: CombatSide) -> bool {
    debuffs::ticks_down(power, amount, side)
}
