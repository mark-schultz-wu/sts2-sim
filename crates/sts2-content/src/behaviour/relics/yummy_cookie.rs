//! `YummyCookie`: four of the deck's cards upgraded as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect};

/// `CardsVar(4)`.
const CARDS: usize = 4;

/// `AfterObtained`: `FromDeckForUpgrade` of four, then `CardCmd.Upgrade` on
/// each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Upgrade,
            count: CARDS,
        });
    }
}
