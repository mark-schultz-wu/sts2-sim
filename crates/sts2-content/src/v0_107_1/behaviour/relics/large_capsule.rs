//! `LargeCapsule`: two relics, then a Strike and a Defend.

use sts2_core::{CardTable as _, CardTagKinds as _, EventHook, HookEffect, RelicPull, Table as _};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardRarity, CardTag, Cards, pool_cards};
use super::super::super::generated::characters::Character;

/// Its `Relics`, pulled from the front of the bag.
const RELICS: usize = 2;

/// `AfterObtained`: `RelicFactory.PullNextRelicFromFront` twice, each
/// obtained, then the character's basic Strike and Defend into the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { character, .. } = hook {
        for _ in 0..RELICS {
            asked.push(HookEffect::ObtainNextRelic(RelicPull::Rolled));
        }
        asked.push(HookEffect::AddToDeck(basic(character, CardTag::STRIKE)));
        asked.push(HookEffect::AddToDeck(basic(character, CardTag::DEFEND)));
    }
}

/// `CardPool.AllCards.First`, over the `Basic` cards carrying `tag`.
fn basic(character: Character, tag: CardTag) -> Cards {
    pool_cards(character.data().card_pool)
        .iter()
        .copied()
        .find(|card| card.rarity() == CardRarity::Basic && card.tagged(tag))
        .unwrap_or_else(|| unimplemented!("a {character:?} pool with no basic {tag:?}"))
}
