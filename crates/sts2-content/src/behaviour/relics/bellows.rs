//! `Bellows`: the opening hand upgraded.

use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart` on its owner's first turn: every card in hand
/// upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::UpgradeHand);
    }
}
