//! `PreservedFog`: three cards removed, and Folly in the deck.

use sts2_core::{DeckAct, EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval` for three, each
/// removed, then `CardPileCmd.AddCurseToDeck<Folly>`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::SelectFromDeck {
                act: DeckAct::Remove,
                count: CARDS,
            },
            HookEffect::AddToDeck(V::Card::FOLLY),
        ]);
    }
}
