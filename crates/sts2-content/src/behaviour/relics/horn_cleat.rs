//! `HornCleat`: block as the second turn begins.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterBlockCleared`: its block once its owner's block clears on the second
/// turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BlockCleared { creature, turn } = hook
        && creature == owner
        && turn == 2
    {
        asked.push(HookEffect::GainBlock(V::Relic::HORN_CLEAT_BLOCK));
    }
}
