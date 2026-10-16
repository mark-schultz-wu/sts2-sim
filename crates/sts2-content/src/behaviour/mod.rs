//! What content does, written once for every build.
//!
//! Generic over the build, and filed the way a build's own behaviour is. A
//! rule moves here once it names content through the build. A rule a later
//! build changed stays in each build that has it.

pub(crate) mod acts;
pub(crate) mod enchantments;
pub(crate) mod encounters;
pub(crate) mod modifiers;
pub(crate) mod orbs;
pub(crate) mod potions;

use sts2_core::{CardAt, CardKind, Change, CostPool, Fight, Lasting, Pile, RunStreamKinds};

/// What a fight's card writes on its deck version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckWrite {
    /// `BuffFromPlay(n)` of `GeneticAlgorithm` or `TheScythe`.
    BuffFromPlay(i32),
    /// `DeckVersion.Enchantment.Amount += n`, Goopy's.
    RaiseEnchantment(u8),
}

/// `CardModel.SetToFreeThisTurn`: energy and stars at nothing this turn or
/// until played.
pub(crate) fn free_this_turn<V: crate::Build, F: Fight<V>>(fight: &mut F, at: CardAt<V>) {
    for pool in [CostPool::Energy, CostPool::Star] {
        fight.modify_card(
            at,
            Change::SetCost(pool, num_traits::zero()),
            Lasting::ThisTurnOrUntilPlayed,
        );
    }
}

/// `GetDistinctForCombat` of one card of the character's pool that `keeps`
/// holds, off `CombatCardGeneration`, then `SetToFreeThisTurn` and into the
/// hand. Nothing when the pool holds none.
///
/// `generated` is `CardPileCmd.AddGeneratedCardToCombat`, which the fight
/// records and raises `AfterCardGeneratedForCombat` for. Otherwise it is a
/// plain `CardPileCmd.Add`.
pub(crate) fn free_card_into_hand<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    keeps: impl Fn(V::Card) -> bool,
    generated: bool,
) {
    let mut pool: Vec<V::Card> = V::combat_cards(fight.character())
        .iter()
        .copied()
        .filter(|&card| keeps(card))
        .collect();
    if pool.is_empty() {
        return;
    }
    let stream = <V::RunRngType as RunStreamKinds>::COMBAT_CARD_GENERATION;
    let [made] = fight
        .generate_cards::<1>(&mut pool, stream)
        .unwrap_or_else(|_| unreachable!("a pool with a card in it makes one"));
    let at = match generated {
        true => fight
            .add_generated_cards(made, Pile::Hand, 1)
            .first()
            .copied(),
        false => fight.add_card(made, Pile::Hand),
    };
    if let Some(at) = at {
        free_this_turn(fight, at);
    }
}

/// The character's combat pool, narrowed to the cards of `kind`. A card
/// whose type the fight works out is in none.
pub(crate) fn character_cards<V: crate::Build, F: Fight<V>>(
    fight: &F,
    kind: CardKind,
) -> Vec<V::Card> {
    V::combat_cards(fight.character())
        .iter()
        .copied()
        .filter(|&card| V::card_kind(card) == Some(kind))
        .collect()
}
