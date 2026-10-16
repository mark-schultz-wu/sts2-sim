//! `Bookmark`: a retained card one cheaper until played.

use sts2_core::{EventHook, HookEffect};

/// `AfterFlush`: one retained card that costs energy, picked off
/// `CombatCardSelection`, costs one less until played. A fight here has one
/// player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Flushed = hook {
        asked.push(HookEffect::LowerRetainedCost);
    }
}
