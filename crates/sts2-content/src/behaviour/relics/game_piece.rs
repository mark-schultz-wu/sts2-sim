//! `GamePiece`: a card drawn for every Power played.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardPlayed`: `CardPileCmd.Draw` for each Power its owner plays.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, player, .. } = hook
        && player == owner
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Power)
    {
        asked.push(HookEffect::Draw(V::Relic::GAME_PIECE_CARDS));
    }
}
