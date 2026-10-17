//! `Anchor`: the block a fight opens with.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeCombatStart`, which is `CreatureCmd.GainBlock` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatStarting { .. } = hook {
        asked.push(HookEffect::GainBlock(V::Relic::ANCHOR_BLOCK));
    }
}
