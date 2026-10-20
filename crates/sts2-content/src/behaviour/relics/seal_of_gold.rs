//! `SealOfGold`: gold spent for energy as each of the player's turns starts.

use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;
use crate::vars::RelicVars as _;

/// `AfterSideTurnStart`, on the side that holds its owner, the one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        ..
    } = hook
    {
        asked.push(HookEffect::GainEnergyForGold {
            energy: V::Relic::SEAL_OF_GOLD_ENERGY,
            gold: V::Relic::SEAL_OF_GOLD_GOLD,
        });
    }
}
