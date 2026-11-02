//! `LostWisp`: damage to every enemy after each Power its owner plays.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardPlayed`: `CreatureCmd.Damage` on every enemy the player can hit,
/// once a Power's play is over.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, player, .. } = hook
        && player == owner
        && V::card_kind(card) == Some(CardKind::Power)
    {
        asked.push(HookEffect::DamageOpponents(V::Relic::LOST_WISP_DAMAGE));
    }
}
