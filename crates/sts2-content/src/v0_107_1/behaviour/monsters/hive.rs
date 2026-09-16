//! The second act's monsters.

use sts2_core::{
    Amount, CardAt, CardTable as _, Effect, Fight, Pile, PowerAmount, RunStreamKinds as _, Scaling,
};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::cards::CardRarity;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// Powers a Hive monster applies to itself as it enters combat.
pub(super) fn starting_powers(
    monster: Monster,
    _ascension: AscensionLevel,
) -> Option<(Power, PowerAmount<Number>)> {
    match monster {
        Monster::ThievingHopper => Some((Power::EscapeArtistPower, PowerAmount::Counted(5))),
        _ => None,
    }
}

/// Perform one of the Hive monsters' moves.
pub(super) fn perform<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ThieveryMove => thievery(fight),
        MoveId::BiteMove | MoveId::BelowMove => fight.attack_opponents(),
        MoveId::BurrowMove => burrow(fight),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// Hide underground behind block that persists until it is broken.
fn burrow<F: Fight<V>>(fight: &mut F) {
    const BLOCK: Number = 32;
    const TOUGH_BLOCK: Number = 37;

    fight.apply_to_actor(Power::BurrowedPower, PowerAmount::Counted(1));
    let amount = match fight.ascension().has(AscensionLevel::ToughEnemies) {
        true => TOUGH_BLOCK,
        false => BLOCK,
    };
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Number(amount),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// Take the highest-priority deck card still in combat, then attack.
fn thievery<F: Fight<V>>(fight: &mut F) {
    let cards: Vec<CardAt<V>> = [Pile::Draw, Pile::Discard]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .collect();
    let priorities = [
        &[CardRarity::Uncommon][..],
        &[CardRarity::Common, CardRarity::Rare, CardRarity::Event][..],
        &[CardRarity::Basic, CardRarity::Quest][..],
        &[CardRarity::Ancient][..],
    ];
    let candidates = priorities
        .into_iter()
        .find_map(|rarities| {
            let matching: Vec<_> = cards
                .iter()
                .copied()
                .filter(|&card| {
                    let ident = fight.card_ident(card);
                    let imbued = ident.enchantment == Some(Enchantment::Imbued);
                    let priority_accepts_imbued = rarities == [CardRarity::Ancient];
                    (imbued == priority_accepts_imbued)
                        && (rarities.contains(&ident.card.rarity()) || imbued)
                })
                .collect();
            (!matching.is_empty()).then_some(matching)
        })
        .unwrap_or(cards);
    if let Some(card) = fight.random_card(
        &candidates,
        <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
    ) {
        fight.remove_card(card);
        fight.apply_to_actor(Power::SwipePower, PowerAmount::Counted(1));
    }
    fight.attack_opponents();
}
