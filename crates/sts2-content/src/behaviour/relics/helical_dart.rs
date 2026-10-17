//! `HelicalDart`: Dexterity for the turn after each Shiv its owner plays.

use crate::vars::RelicVars as _;
use sts2_core::{CardTable as _, EventHook, HookEffect, PowerAmount};

use crate::generated::cards::CardTag;
use crate::ids::PowerIds as _;

/// `AfterCardPlayed`: `HelicalDartPower` on its owner once a Shiv's play is
/// over. A relic's owner is the player, and a fight here has one, so every
/// card played is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, .. } = hook
        && card.tagged(CardTag::Shiv)
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::HELICAL_DART_POWER,
            PowerAmount::Counted(V::Relic::HELICAL_DART_DEXTERITY_POWER),
        ));
    }
}
