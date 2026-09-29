//! `HelicalDart`: Dexterity for the turn after each Shiv its owner plays.

use sts2_core::{CardTable as _, EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<DexterityPower>(1)`.
const DEXTERITY: Number = 1;

/// `AfterCardPlayed`: `HelicalDartPower` on its owner once a Shiv's play is
/// over. A relic's owner is the player, and a fight here has one, so every
/// card played is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CardPlayed { card, .. } = hook
        && card.tagged(CardTag::Shiv)
    {
        asked.push(HookEffect::ApplyToOwner(
            Power::HelicalDartPower,
            PowerAmount::Counted(DEXTERITY),
        ));
    }
}
