//! `CloakClasp`: block for each card left in hand as the turn ends.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `BlockVar(1, Unpowered)`, for each card in hand.
const BLOCK: Number = 1;

/// `BeforeSideTurnEnd` on its owner's side: its block for each card in
/// hand, when the hand holds any.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::BeforeSideTurnEnd {
        side: CombatSide::Player,
        hand,
        ..
    } = hook
        && hand != 0
    {
        let cards = Number::try_from(hand).expect("a hand's size fits the game's numbers");
        asked.push(HookEffect::GainBlock(cards * BLOCK));
    }
}
