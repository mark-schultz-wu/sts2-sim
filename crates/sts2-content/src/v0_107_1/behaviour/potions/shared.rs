//! The potions every character's pool draws from.

use sts2_core::{
    Change, CostPool, Fight, Lasting, Pile, Played, PowerAmount, RunStreamKinds as _, Table as _,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardType, Cards, combat_cards};
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `DexterityPotion`'s `PowerVar<DexterityPower>(2)`.
const DEXTERITY: Number = 2;

/// `SpeedPotion`'s `PowerVar<DexterityPower>(5)`, lent until the turn ends.
const SPEED: Number = 5;

/// `StrengthPotion`'s `PowerVar<StrengthPower>(2)`.
const STRENGTH: Number = 2;

/// `VulnerablePotion`'s `PowerVar<VulnerablePower>(3)`.
const VULNERABLE: Number = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    PowerPotionChoice,
    AttackPotionChoice,
}

/// What one of the shared potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyPlayer`, which a run with one player points at that player.
        Potion::DexterityPotion => {
            fight.apply_to_actor(Power::DexterityPower, PowerAmount::Counted(DEXTERITY));
            Played::Done
        }
        // `AnyPlayer` again.
        Potion::StrengthPotion => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(STRENGTH));
            Played::Done
        }
        // `AnyPlayer`, and `SpeedPotionPower` lends the Dexterity.
        Potion::SpeedPotion => {
            fight.apply_to_actor(Power::SpeedPotionPower, PowerAmount::Counted(SPEED));
            Played::Done
        }
        Potion::VulnerablePotion => {
            fight.apply_power(
                fight.target(),
                Power::VulnerablePower,
                PowerAmount::Counted(VULNERABLE),
            );
            Played::Done
        }
        // `AnyPlayer` again, and a run with one player walks that player's
        // hand. Every card in it that can be upgraded is.
        Potion::BlessingOfTheForge => {
            for card in fight.cards_in(Pile::Hand) {
                fight.modify_card(
                    card,
                    sts2_core::Change::Upgrade,
                    sts2_core::Lasting::ThisFight,
                );
            }
            Played::Done
        }
        Potion::PowerPotion => free_card_of(fight, CardType::Power, Resume::PowerPotionChoice),
        Potion::AttackPotion => free_card_of(fight, CardType::Attack, Resume::AttackPotionChoice),
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// `PowerPotion` and `AttackPotion`: three distinct cards of `kind` from the
/// character's pool, off `CombatCardGeneration`. The player takes one into
/// hand, free this turn, or none.
fn free_card_of<F: Fight<V>>(fight: &mut F, kind: CardType, at: Resume) -> Played<V> {
    let pool = fight.character().data().card_pool;
    let mut cards: Vec<Cards> = combat_cards(pool)
        .iter()
        .copied()
        .filter(|card| card.data().kind == crate::Field::Constant(kind))
        .collect();
    let offered = fight
        .generate_cards(
            &mut cards,
            <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
        )
        .unwrap_or_else(|_| unimplemented!("a character pool with too few {kind:?} cards"));
    Played::Choosing {
        offered,
        skippable: true,
        at: super::super::Resume::Potion(super::Resume::Shared(at)),
    }
}

pub(super) fn resume<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    answer: sts2_core::Taken<V>,
) -> Played<V> {
    match (at, answer) {
        (
            Resume::PowerPotionChoice | Resume::AttackPotionChoice,
            sts2_core::Taken::Chosen(chosen),
        ) => {
            if let Some(card) = chosen {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    for pool in [CostPool::Energy, CostPool::Star] {
                        fight.modify_card(
                            at,
                            Change::SetCost(pool, num_traits::zero()),
                            Lasting::ThisTurnOrUntilPlayed,
                        );
                    }
                }
            }
            Played::Done
        }
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}
