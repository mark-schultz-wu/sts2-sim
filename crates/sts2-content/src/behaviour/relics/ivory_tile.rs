//! `IvoryTile`: energy back for a card that spent three or more.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardPlayed`: `PlayerCmd.GainEnergy` for a play of its owner's
/// that spent the threshold or more. A fight here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { spent, .. } = hook
        && spent >= (V::Relic::IVORY_TILE_ENERGY_THRESHOLD as u32)
    {
        asked.push(HookEffect::GainEnergy(V::Relic::IVORY_TILE_ENERGY));
    }
}
