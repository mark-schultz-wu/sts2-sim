//! `GhostSeed`: its owner's basic Strikes and Defends are Ethereal.

use sts2_core::{CardTable, EventHook, HookEffect, Table as _, Writing};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardKeyword, CardRarity, CardTag, Cards};
use super::super::super::generated::encounters::RoomType;

/// `CanAffect`: a basic Strike or Defend. Ethereal written on twice stays
/// once, so the keyword check it also makes changes nothing written.
fn affects(card: Cards) -> bool {
    card.rarity() == CardRarity::Basic
        && (card.tagged(CardTag::Strike) || card.tagged(CardTag::Defend))
}

/// `AfterCardEnteredCombat` writes Ethereal on a card it affects as the card
/// arrives. `AfterRoomEntered` writes it on every one the fight holds as a
/// fight's room opens.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardEnteredCombat { at, card, .. } if affects(card) => {
            asked.push(HookEffect::AddKeyword {
                to: Writing::One(at),
                keyword: CardKeyword::Ethereal,
            });
        }
        EventHook::RoomEntered {
            room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
            ..
        } => {
            for &card in Cards::ALL.iter().filter(|&&card| affects(card)) {
                asked.push(HookEffect::AddKeyword {
                    to: Writing::EveryCopyOf(card),
                    keyword: CardKeyword::Ethereal,
                });
            }
        }
        _ => {}
    }
}
