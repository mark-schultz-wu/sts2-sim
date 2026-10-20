//! `Pomander`: one card of the deck upgraded as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForUpgrade`, then `CardCmd.Upgrade`
/// on each card it takes.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Upgrade,
            count: (V::Relic::POMANDER_CARDS as usize),
        });
    }
}
