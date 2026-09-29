//! `BingBong`: a copy of every card that joins the deck.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `AfterCardChangedPiles`: a clone of a card that lands in the deck, at the
/// deck's bottom. A clone some listener made is passed over, and so is its
/// own clone, which it also keeps in `CardsToSkip`. A pile in a fight is never
/// the deck. A relic's owner is the player, and a run here has one, so every
/// card is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::AddedToDeck { cloned: false, .. } => asked.push(HookEffect::CloneIntoDeck),
        EventHook::AddedToDeck { cloned: true, .. }
        | EventHook::CardChangedPiles { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BingBong does for {hook:?}"),
    }
}
