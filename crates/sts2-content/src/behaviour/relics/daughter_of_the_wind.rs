//! `DaughterOfTheWind`: a point of block after each attack its owner plays.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BlockVar(1, ValueProp.Unpowered)`.
const BLOCK: Number = 1;

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
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
