//! The potions only the Regent's pool draws.

use sts2_core::{Fight, Played};

use crate::behaviour::potions::Number;
use crate::ids::{CardIds as _, PotionIds as _};

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
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `PlayerCmd.GainStars` by its `Stars`.
        __c0 if __c0 == V::Potion::STAR_POTION => {
            fight.gain_stars(STARS);
            Played::Done
        }
        // `AnyPlayer`: `ForgeCmd.Forge` for its `Forge`.
        __c0 if __c0 == V::Potion::KINGS_COURAGE => {
            fight.forge(V::Card::SOVEREIGN_BLADE, KINGS_COURAGE);
            Played::Done
        }
        // `Self`: `GetDistinctForCombat` of three colorless cards off
        // `CombatCardGeneration`, each upgraded and into hand.
        __c0 if __c0 == V::Potion::COSMIC_CONCOCTION => {
            use sts2_core::{Change, Lasting, Pile, RunStreamKinds as _};

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
