//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod acts;
pub(crate) mod cards;
pub(crate) mod enchantments;
pub(crate) mod encounters;
pub mod events;
pub(crate) mod monsters;
pub(crate) mod orbs;
pub(crate) mod potions;
pub(crate) mod powers;
pub(crate) mod relics;

use sts2_core::{Fight, Played, Table as _, Taken};

use super::V0_107_1 as V;
use super::generated::cards::{CardType, Cards, combat_cards};

/// `CardModel.SetToFreeThisTurn`: energy and stars at nothing this turn or
/// until played.
pub(crate) fn free_this_turn<F: Fight<V>>(fight: &mut F, at: sts2_core::CardAt<V>) {
    for pool in [sts2_core::CostPool::Energy, sts2_core::CostPool::Star] {
        fight.modify_card(
            at,
            sts2_core::Change::SetCost(pool, num_traits::zero()),
            sts2_core::Lasting::ThisTurnOrUntilPlayed,
        );
    }
}

/// `GetDistinctForCombat` of one card of the character's pool that `keeps`
/// holds, off `CombatCardGeneration`, then `SetToFreeThisTurn` and
/// `CardPileCmd.Add` into the hand. Nothing when the pool holds none.
pub(crate) fn free_card_into_hand<F: Fight<V>>(fight: &mut F, keeps: impl Fn(Cards) -> bool) {
    let mut pool: Vec<Cards> = <V as sts2_core::Rules>::combat_cards(fight.character())
        .iter()
        .copied()
        .filter(|&card| keeps(card))
        .collect();
    if pool.is_empty() {
        return;
    }
    let stream = <<V as sts2_core::ContentVersion>::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
    let [made] = fight
        .generate_cards::<1>(&mut pool, stream)
        .unwrap_or_else(|_| unreachable!("a pool with a card in it makes one"));
    if let Some(at) = fight.add_card(made, sts2_core::Pile::Hand) {
        free_this_turn(fight, at);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Card(cards::Resume),
    Potion(potions::Resume),
    Monster(monsters::Resume),
}

pub(crate) fn resume<F: Fight<V>>(fight: &mut F, at: Resume, answer: Taken<V>) -> Played<V> {
    match (at, answer) {
        (Resume::Card(at), Taken::Selected(cards)) => cards::resume_play(fight, at, cards),
        (Resume::Potion(at), answer) => potions::resume(fight, at, answer),
        (Resume::Monster(at), Taken::Chosen(card)) => monsters::resume(fight, at, card),
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}

/// The character's combat pool, narrowed to the cards of `kind`.
pub(crate) fn character_cards<F: Fight<V>>(fight: &F, kind: CardType) -> Vec<Cards> {
    let pool = fight.character().data().card_pool;
    combat_cards(pool)
        .iter()
        .copied()
        .filter(|card| card.data().kind == crate::Field::Constant(kind))
        .collect()
}
