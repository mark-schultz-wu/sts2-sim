//! `NeowsTorment`: Neow's Fury in the deck.

use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`, which adds one Neow's Fury to the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(V::Card::NEOWS_FURY));
    }
}
