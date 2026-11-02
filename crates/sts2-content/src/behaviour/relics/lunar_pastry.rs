//! `LunarPastry`: a star as each of its owner's turns ends.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnEnd` for the player's side: `PlayerCmd.GainStars`.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnEnd { participants, .. } = hook
        && participants.contains(owner)
    {
        asked.push(HookEffect::GainStars(V::Relic::LUNAR_PASTRY_STARS));
    }
}
