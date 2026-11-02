//! `Sai`: block as each of its owner's turns begins.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnStart`: its block on its owner, for the side its owner is
/// among.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart { participants, .. } = hook
        && participants.contains(owner)
    {
        asked.push(HookEffect::GainBlock(V::Relic::SAI_BLOCK));
    }
}
