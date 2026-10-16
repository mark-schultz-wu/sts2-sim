//! `Lantern`: an energy on the first turn of a fight.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AfterSideTurnStart`: its energy once its owner's first turn has drawn.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::GainEnergy(ENERGY));
    }
}
