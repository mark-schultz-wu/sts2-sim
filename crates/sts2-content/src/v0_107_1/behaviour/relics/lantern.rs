//! `Lantern`: an energy on the first turn of a fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AfterSideTurnStart`: its energy once its owner's first turn has drawn.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
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
