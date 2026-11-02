//! `Toolbox`: a colorless card of three, chosen into the first hand.

use sts2_core::{EventHook, HookEffect};

/// `BeforeHandDraw` on its owner's first turn: three distinct colorless
/// cards offered, and the one chosen into hand. Its `CardsVar(3)` is the
/// three a choice offers.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::BeforeHandDraw { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::BeforeHandDraw { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::ChooseColorlessIntoHand);
    }
}
