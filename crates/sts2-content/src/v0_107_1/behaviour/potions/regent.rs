//! The potions only the Regent's pool draws.

use sts2_core::{Fight, Played};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::Number;

/// `KingsCourage`'s `ForgeVar(15)`.
const KINGS_COURAGE: Number = 15;

/// `CosmicConcoction`'s `CardsVar(3)`.
const COSMIC_CARDS: usize = 3;

/// `StarPotion`'s `StarsVar(3)`.
const STARS: Number = 3;

/// What one of the Regent's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `PlayerCmd.GainStars` by its `Stars`.
        Potion::StarPotion => {
            fight.gain_stars(STARS);
            Played::Done
        }
        // `AnyPlayer`: `ForgeCmd.Forge` for its `Forge`.
        Potion::KingsCourage => {
            use super::super::super::generated::cards::{CardVar, Cards, TokenCard};

            fight.forge(
                Cards::Token(TokenCard::SovereignBlade),
                CardVar::Damage,
                KINGS_COURAGE,
            );
            Played::Done
        }
        // `Self`: `GetDistinctForCombat` of three colorless cards off
        // `CombatCardGeneration`, each upgraded and into hand.
        Potion::CosmicConcoction => {
            use sts2_core::{Change, Lasting, Pile, Rules as _, RunStreamKinds as _};

            let mut cards = V::combat_colorless_cards().to_vec();
            let made: [_; COSMIC_CARDS] = fight
                .generate_cards(
                    &mut cards,
                    <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
                )
                .unwrap_or_else(|_| unreachable!("the colorless pool holds far more than three"));
            for card in made {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
                }
            }
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
