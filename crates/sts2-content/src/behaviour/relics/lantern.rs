//! `Lantern`: an energy on the first turn of a fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnStart`: its energy once its owner's first turn has drawn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn <= 1
    {
        asked.push(HookEffect::GainEnergy(V::Relic::LANTERN_ENERGY));
    }
}
