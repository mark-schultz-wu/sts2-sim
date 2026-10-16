//! `PaelsHorn`: two Relax into the deck as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `CardPileCmd.Add` of a Relax to the deck, twice.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([HookEffect::AddToDeck(V::Card::RELAX); 2]);
    }
}
