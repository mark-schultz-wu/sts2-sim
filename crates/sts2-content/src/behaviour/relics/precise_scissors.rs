//! `PreciseScissors`: one card out of the deck as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval`, then each card it
/// takes comes out of the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: (V::Relic::PRECISE_SCISSORS_CARDS as usize),
        });
    }
}
