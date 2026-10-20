//! `Storybook`: a Brightest Flame into the deck as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `CardPileCmd.Add` of a Brightest Flame to the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(V::Card::BRIGHTEST_FLAME));
    }
}
