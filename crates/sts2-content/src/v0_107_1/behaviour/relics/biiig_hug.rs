//! `BiiigHug`: four cards out of the deck, and Soot in the draw pile after
//! every shuffle.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, StatusCard};

/// `CardsVar(4)`.
const CARDS: usize = 4;

/// A relic's owner is the player, and a fight here has one, so every
/// shuffle is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        // `AfterObtained`: `FromDeckForRemoval` of four, then
        // `CardPileCmd.RemoveFromDeck`.
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: CARDS,
        }),
        // `AfterShuffle`: a Soot into the draw pile at a random place.
        EventHook::Shuffled => asked.push(HookEffect::AddToDrawAtRandom {
            card: Cards::Status(StatusCard::Soot),
            count: 1,
            by_owner: true,
        }),
        _ => {}
    }
}
