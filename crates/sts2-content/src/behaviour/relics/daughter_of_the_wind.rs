//! `DaughterOfTheWind`: a point of block after each attack its owner plays.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardPlayed`: `CreatureCmd.GainBlock` on its owner once an attack's
/// play is over. A relic's owner is the player, and a fight here has one, so
/// every card played is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, .. } = hook
        && V::card_kind(card) == Some(CardKind::Attack)
    {
        asked.push(HookEffect::GainBlock(V::Relic::DAUGHTER_OF_THE_WIND_BLOCK));
    }
}
