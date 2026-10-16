//! `BingBong`: a copy of every card that joins the deck.

use sts2_core::{EventHook, HookEffect};

/// `CardsToSkip`: the copies it made that have not joined the deck again
/// from no pile since.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    skip: Vec<sts2_core::DeckCardId>,
}

/// `AfterCardChangedPiles`: a copy of a card that lands in the deck, at the
/// deck's bottom, with `clonedBy` itself. A card some listener cloned is
/// passed over. Its own copies join that way, so they stay in `skip`, and one
/// that joins again from no pile is taken off and not copied. A pile in a
/// fight is never the deck, and every card is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::AddedToDeck {
            id, cloned: false, ..
        } => match state.skip.iter().position(|&held| held == id) {
            Some(at) => {
                state.skip.remove(at);
            }
            None => asked.push(HookEffect::CopyIntoDeck(1)),
        },
        // `CardsToSkip.Add` of the copy its own `CloneCard` made.
        EventHook::CopyMade { id } => state.skip.push(id),
        EventHook::AddedToDeck { cloned: true, .. }
        | EventHook::CardChangedPiles { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BingBong does for {hook:?}"),
    }
}
