//! `LunarPastry`: a star as each of its owner's turns ends.

use sts2_core::{EventHook, HookEffect, SideKinds as _};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `StarsVar(1)`.
const STARS: Number = 1;

/// `AfterSideTurnEnd` for the player's side: `PlayerCmd.GainStars`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnEnd { side, .. } = hook
        && side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER
    {
        asked.push(HookEffect::GainStars(STARS));
    }
}
