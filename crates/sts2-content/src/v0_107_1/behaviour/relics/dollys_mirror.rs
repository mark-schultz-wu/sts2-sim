//! `DollysMirror`: the deck card its pickup copies.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// Nothing in a fight. Its one override is `AfterObtained`, which offers the
/// deck and adds a clone of the chosen card.
///
/// `Filter` is every card but a quest card, so a quest card is in the deck and
/// not in the selection.
pub(super) fn event_hook(hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::Obtained => vec![HookEffect::CloneDeckCard {
            excluding: CardKind::Quest,
        }],
        _ => Vec::new(),
    }
}
