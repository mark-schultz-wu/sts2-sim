//! `CaptainsWheel`: block as the third turn's block clears.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterBlockCleared` for its owner on turn three: `CreatureCmd.GainBlock`.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BlockCleared { creature, turn } = hook
        && creature == owner
        && turn == 3
    {
        asked.push(HookEffect::GainBlock(V::Relic::CAPTAINS_WHEEL_BLOCK));
    }
}
