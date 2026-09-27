//! `LostWisp`: damage to every enemy after each Power its owner plays.

use sts2_core::{CardKind, EventHook, HookEffect, Rules as _};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DamageVar(8, ValueProp.Unpowered)`.
const DAMAGE: Number = 8;

/// `AfterCardPlayed`: `CreatureCmd.Damage` on every enemy the player can
/// hit, once a Power's play is over. A relic's owner is the player, and a
/// fight here has one, so every card played is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CardPlayed { card, .. } = hook
        && V::card_kind(card) == Some(CardKind::Power)
    {
        asked.push(HookEffect::DamageOpponents(DAMAGE));
    }
}
