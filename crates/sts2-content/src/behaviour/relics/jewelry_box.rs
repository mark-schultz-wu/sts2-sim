//! `JewelryBox`: an Apotheosis in the deck.

use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `CardPileCmd.Add` of an Apotheosis to the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(V::Card::APOTHEOSIS));
    }
}
