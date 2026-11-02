//! `SealOfGold`: gold spent for energy as each of the player's turns starts.

use sts2_core::{EventHook, HookEffect};

use crate::vars::RelicVars as _;

/// `AfterSideTurnStart`, where its owner takes part.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart { participants, .. } = hook
        && participants.contains(owner)
    {
        asked.push(HookEffect::GainEnergyForGold {
            energy: V::Relic::SEAL_OF_GOLD_ENERGY,
            gold: V::Relic::SEAL_OF_GOLD_GOLD,
        });
    }
}
