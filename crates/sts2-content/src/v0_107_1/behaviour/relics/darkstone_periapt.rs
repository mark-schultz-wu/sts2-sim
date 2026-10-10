//! `DarkstonePeriapt`: maximum health for each Curse that joins the deck.

use sts2_core::{CardKind, EventHook, HookEffect, Rules as _};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(6)`.
const MAX_HP: Number = 6;

/// `AfterCardChangedPiles`: its maximum health for a Curse now in the deck.
/// A fight moves cards between its own piles alone, none of them the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::AddedToDeck { card, .. } => {
            if V::card_kind(card) == Some(CardKind::Curse) {
                asked.push(HookEffect::GainMaxHp(MAX_HP));
            }
        }
        EventHook::CardChangedPiles { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DarkstonePeriapt does for {hook:?}"),
    }
}
