//! `PowerCell`: two cards that cost nothing, from the draw pile into the
//! first hand.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeSideTurnStart` for the player's side on its first turn: the draw
/// pile's cards that cost nothing, shuffled, and the first two into hand.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeSideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn <= 1
    {
        asked.push(HookEffect::ZeroCostFromDraw(
            V::Relic::POWER_CELL_CARDS as usize,
        ));
    }
}
