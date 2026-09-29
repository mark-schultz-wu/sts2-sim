//! `PreciseScissors`: one card out of the deck as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `CardsVar(1)`.
const CARDS: usize = 1;

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval`, then each card it
/// takes comes out of the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: CARDS,
        });
    }
}
