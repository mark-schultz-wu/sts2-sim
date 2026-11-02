//! `GhostSeed`: its owner's basic Strikes and Defends are Ethereal.

use sts2_core::{CardTable, EventHook, HookEffect, Writing};

use crate::generated::cards::{CardKeyword, CardRarity, CardTag};
use crate::generated::encounters::RoomType;

/// `CanAffect`: a basic Strike or Defend. Ethereal written on twice stays
/// once, so the keyword check it also makes changes nothing written.
fn affects<V: crate::Build>(card: V::Card) -> bool {
    card.rarity() == CardRarity::Basic
        && (card.tagged(CardTag::Strike) || card.tagged(CardTag::Defend))
}

/// `AfterCardEnteredCombat` writes Ethereal on a card it affects as the card
/// arrives. `AfterRoomEntered` writes it on every one the fight holds as a
/// fight's room opens.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardEnteredCombat {
            at, card, player, ..
        } if player == owner && affects::<V>(card) => {
            asked.push(HookEffect::AddKeyword {
                to: Writing::One(at),
                keyword: CardKeyword::Ethereal,
            });
        }
        EventHook::RoomEntered {
            room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
            ..
        } => {
            for &card in <V::Card as sts2_core::Table>::ALL
                .iter()
                .filter(|&&card| affects::<V>(card))
            {
                asked.push(HookEffect::AddKeyword {
                    to: Writing::EveryCopyOf(card),
                    keyword: CardKeyword::Ethereal,
                });
            }
        }
        _ => {}
    }
}
