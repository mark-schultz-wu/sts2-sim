//! `EmptyCage`: two cards out of the deck as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval` for two, then
/// `CardPileCmd.RemoveFromDeck` on each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: (V::Relic::EMPTY_CAGE_CARDS as usize),
        });
    }
}
