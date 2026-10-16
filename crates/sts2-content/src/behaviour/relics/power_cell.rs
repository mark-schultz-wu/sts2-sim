//! `PowerCell`: two cards that cost nothing, from the draw pile into the
//! first hand.

use sts2_core::{EventHook, HookEffect, SideKinds as _};

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `BeforeSideTurnStart` for the player's side on its first turn: the draw
/// pile's cards that cost nothing, shuffled, and the first two into hand.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeSideTurnStart { side, turn, .. } = hook
        && side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER
        && turn <= 1
    {
        asked.push(HookEffect::ZeroCostFromDraw(CARDS));
    }
}
