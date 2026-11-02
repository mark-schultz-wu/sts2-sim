//! `Bellows`: the opening hand upgraded.

use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart` on its owner's first turn: every card in hand
/// upgraded.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::UpgradeHand);
    }
}
