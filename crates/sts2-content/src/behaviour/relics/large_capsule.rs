//! `LargeCapsule`: two relics, then a Strike and a Defend.

use crate::vars::RelicVars as _;
use sts2_core::{CardTable as _, CardTagKinds as _, EventHook, HookEffect, RelicPull};

use crate::generated::cards::{CardRarity, CardTag};

/// `AfterObtained`: `RelicFactory.PullNextRelicFromFront` twice, each
/// obtained, then the character's basic Strike and Defend into the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { character, .. } = hook {
        for _ in 0..(V::Relic::LARGE_CAPSULE_RELICS as usize) {
            asked.push(HookEffect::ObtainNextRelic(RelicPull::Rolled));
        }
        asked.push(HookEffect::AddToDeck(basic::<V>(
            character,
            CardTag::STRIKE,
        )));
        asked.push(HookEffect::AddToDeck(basic::<V>(
            character,
            CardTag::DEFEND,
        )));
    }
}

/// `CardPool.AllCards.First`, over the `Basic` cards carrying `tag`.
fn basic<V: crate::Build>(character: V::Character, tag: CardTag) -> V::Card {
    V::pool_cards(character)
        .iter()
        .copied()
        .find(|card| card.rarity() == CardRarity::Basic && card.tagged(tag))
        .unwrap_or_else(|| unimplemented!("a {character:?} pool with no basic {tag:?}"))
}
