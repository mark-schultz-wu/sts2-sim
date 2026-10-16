//! `BiiigHug`: four cards out of the deck, and Soot in the draw pile after
//! every shuffle.

use sts2_core::{DeckAct, EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `CardsVar(4)`.
const CARDS: usize = 4;

/// A relic's owner is the player, and a fight here has one, so every
/// shuffle is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        // `AfterObtained`: `FromDeckForRemoval` of four, then
        // `CardPileCmd.RemoveFromDeck`.
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: CARDS,
        }),
        // `AfterShuffle`: a Soot into the draw pile at a random place.
        EventHook::Shuffled => asked.push(HookEffect::AddToDrawAtRandom {
            card: V::Card::SOOT,
            count: 1,
            by_owner: true,
        }),
        _ => {}
    }
}
