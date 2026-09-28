//! `GamePiece`: a card drawn for every Power played.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(1)`.
const CARDS: Number = 1;

/// `AfterCardPlayed`: `CardPileCmd.Draw` for each Power its owner plays. A
/// fight here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CardPlayed { card, .. } = hook
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Power)
    {
        asked.push(HookEffect::Draw(CARDS));
    }
}
