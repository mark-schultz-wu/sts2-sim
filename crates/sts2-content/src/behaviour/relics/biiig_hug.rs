//! `BiiigHug`: four cards out of the deck, and Soot in the draw pile after
//! every shuffle.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

use crate::ids::CardIds as _;

/// A relic's owner is the player, and a fight here has one, so every
/// shuffle is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        // `AfterObtained`: `FromDeckForRemoval` of four, then
        // `CardPileCmd.RemoveFromDeck`.
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Remove,
            count: (V::Relic::BIIIG_HUG_CARDS as usize),
        }),
        // `AfterShuffle` of its owner's: a Soot into the draw pile at a random
        // place.
        EventHook::Shuffled { player } if player == owner => {
            asked.push(HookEffect::AddToDrawAtRandom {
                card: V::Card::SOOT,
                count: 1,
                by_owner: true,
            })
        }
        _ => {}
    }
}
