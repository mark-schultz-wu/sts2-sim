//! `MummifiedHand`: a card in hand free this turn for every Power played.

use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardPlayed`: for a Power its owner plays, one card in hand picked
/// off `CombatCardSelection` and made free this turn. A fight here has one
/// player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, .. } = hook
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Power)
    {
        asked.push(HookEffect::FreeCostlyInHand);
    }
}
