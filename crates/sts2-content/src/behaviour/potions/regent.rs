//! The potions only the Regent's pool draws.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::Fight;

use crate::ids::{CardIds as _, PotionIds as _};

/// What one of the Regent's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(
    potion: V::Potion,
    fight: &mut F,
) -> Played<V, F::Kind> {
    match potion {
        // `PlayerCmd.GainStars` by its `Stars`.
        __c0 if __c0 == V::Potion::STAR_POTION => {
            fight.gain_stars(V::Potion::STAR_POTION_STARS);
            Played::Done
        }
        // `AnyPlayer`: `ForgeCmd.Forge` for its `Forge`.
        __c0 if __c0 == V::Potion::KINGS_COURAGE => {
            fight.forge(V::Card::SOVEREIGN_BLADE, V::Potion::KINGS_COURAGE_FORGE);
            Played::Done
        }
        // `Self`: `GetDistinctForCombat` of three colorless cards off
        // `CombatCardGeneration`, each upgraded and into hand.
        __c0 if __c0 == V::Potion::COSMIC_CONCOCTION => {
            use sts2_core::{Change, Lasting, Pile, RunStreamKinds as _};

            let mut cards = V::combat_colorless_cards(fight.multiplayer()).to_vec();
            // The count is the array's length, which no build's number can
            // set. The assert holds it to the build's `CardsVar`.
            assert_eq!(
                V::Potion::COSMIC_CONCOCTION_CARDS,
                3,
                "Cosmic Concoction's cards"
            );
            let made: [_; 3] = fight
                .generate_cards(
                    &mut cards,
                    <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
                )
                .unwrap_or_else(|_| unreachable!("the colorless pool holds far more than three"));
            // `CardCmd.Upgrade` on each, before it is added.
            for card in made {
                fight.add_generated_written(
                    card,
                    Pile::Hand,
                    1,
                    &[(Change::Upgrade, Lasting::ThisFight)],
                );
            }
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
