//! `HelicalDart`: Dexterity for the turn after each Shiv its owner plays.

use sts2_core::{CardTable as _, EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::cards::CardTag;
use crate::ids::PowerIds as _;

/// `PowerVar<DexterityPower>(1)`.
const DEXTERITY: Number = 1;

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
            PowerAmount::Counted(DEXTERITY),
        ));
    }
}
