//! `LuckyFysh`: gold for each card that joins the deck.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardChangedPiles`: its gold for a card now in the deck, a copy
/// another listener made included.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AddedToDeck { .. } = hook {
        asked.push(HookEffect::GainGold(V::Relic::LUCKY_FYSH_GOLD));
    }
}
