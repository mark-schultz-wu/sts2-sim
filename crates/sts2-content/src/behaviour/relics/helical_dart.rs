//! `HelicalDart`: Dexterity for the turn after each Shiv its owner plays.

use crate::vars::RelicVars as _;
use sts2_core::{CardTable as _, EventHook, HookEffect, PowerAmount};

use crate::generated::cards::CardTag;
use crate::ids::PowerIds as _;

/// `AfterCardPlayed`: `HelicalDartPower` on its owner once a Shiv's play is
/// over.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, player, .. } = hook
        && player == owner
        && card.tagged(CardTag::Shiv)
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::HELICAL_DART_POWER,
            PowerAmount::Counted(V::Relic::HELICAL_DART_DEXTERITY_POWER),
        ));
    }
}
