//! `Bellows`: the opening hand upgraded.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `AfterPlayerTurnStart` on its owner's first turn: every card in hand
/// upgraded.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::UpgradeHand);
    }
}
