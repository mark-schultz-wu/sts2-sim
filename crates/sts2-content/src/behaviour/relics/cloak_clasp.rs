//! `CloakClasp`: block for each card left in hand as the turn ends.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BeforeSideTurnEnd` on its owner's side: its block for each card in
/// hand, when the hand holds any.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeSideTurnEnd {
        participants, hand, ..
    } = hook
        && participants.contains(owner)
        && hand != 0
    {
        let cards = Number::try_from(hand).expect("a hand's size fits the game's numbers");
        asked.push(HookEffect::GainBlock(cards * V::Relic::CLOAK_CLASP_BLOCK));
    }
}
