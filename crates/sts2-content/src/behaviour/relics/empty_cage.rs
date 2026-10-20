//! `EmptyCage`: two cards out of the deck as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect};

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval` for two, then
/// `CardPileCmd.RemoveFromDeck` on each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: CARDS,
        });
    }
}
