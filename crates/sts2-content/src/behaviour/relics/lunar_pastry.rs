//! `LunarPastry`: a star as each of its owner's turns ends.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, SideKinds as _};

/// `AfterSideTurnEnd` for the player's side: `PlayerCmd.GainStars`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnEnd { side, .. } = hook
        && side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER
    {
        asked.push(HookEffect::GainStars(V::Relic::LUNAR_PASTRY_STARS));
    }
}
