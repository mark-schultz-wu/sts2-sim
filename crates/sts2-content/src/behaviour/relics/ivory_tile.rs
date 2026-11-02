//! `IvoryTile`: energy back for a card that spent three or more.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardPlayed`: `PlayerCmd.GainEnergy` for a play of its owner's that
/// spent the threshold or more.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { spent, player, .. } = hook
        && player == owner
        && spent >= (V::Relic::IVORY_TILE_ENERGY_THRESHOLD as u32)
    {
        asked.push(HookEffect::GainEnergy(V::Relic::IVORY_TILE_ENERGY));
    }
}
