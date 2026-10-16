//! `DollysMirror`: the deck card its pickup copies.

use sts2_core::{CardKind, EventHook, HookEffect};

/// Nothing in a fight. Its one override is `AfterObtained`, which offers the
/// deck and adds a clone of the chosen card.
///
/// `Filter` is every card but a quest card, so a quest card is in the deck and
/// not in the selection.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::CloneDeckCard {
            excluding: CardKind::Quest,
        });
    }
}
