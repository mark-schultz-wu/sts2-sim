//! `DarkstonePeriapt`: maximum health for each Curse that joins the deck.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardChangedPiles`: its maximum health for a Curse now in the deck.
/// A fight moves cards between its own piles alone, none of them the deck.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::AddedToDeck { card, player, .. } if player == owner => {
            if V::card_kind(card) == Some(CardKind::Curse) {
                asked.push(HookEffect::GainMaxHp(V::Relic::DARKSTONE_PERIAPT_MAX_HP));
            }
        }
        // `card.Owner == base.Owner`: another player's card does nothing.
        EventHook::AddedToDeck { .. }
        | EventHook::CardChangedPiles { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DarkstonePeriapt does for {hook:?}"),
    }
}
