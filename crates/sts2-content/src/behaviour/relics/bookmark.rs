//! `Bookmark`: a retained card one cheaper until played.

use sts2_core::{EventHook, HookEffect};

/// `AfterFlush` of its owner's hand: one retained card that costs energy,
/// picked off `CombatCardSelection`, costs one less until played.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Flushed { player } = hook
        && player == owner
    {
        asked.push(HookEffect::LowerRetainedCost);
    }
}
