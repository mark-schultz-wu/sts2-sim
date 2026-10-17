//! `YummyCookie`: four of the deck's cards upgraded as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `FromDeckForUpgrade` of four, then `CardCmd.Upgrade` on
/// each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Upgrade,
            count: (V::Relic::YUMMY_COOKIE_CARDS as usize),
        });
    }
}
