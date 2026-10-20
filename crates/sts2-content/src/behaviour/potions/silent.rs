//! The potions only the Silent's pool draws.

use sts2_core::{CardIdent, Change, Fight, Lasting, Pile, Played, PowerAmount};

use crate::behaviour::potions::Number;
use crate::ids::{CardIds as _, PotionIds as _, PowerIds as _};

/// `CunningPotion`'s `CardsVar(3)`.
const CUNNING_SHIVS: usize = 3;

/// `GhostInAJar`'s `PowerVar<IntangiblePower>(1)`.
const GHOST_IN_A_JAR: Number = 1;

/// `PoisonPotion`'s `PowerVar<PoisonPower>(6)`.
const POISON: Number = 6;

/// What one of the Silent's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyEnemy`.
        __c0 if __c0 == V::Potion::POISON_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::POISON_POWER,
                PowerAmount::Counted(POISON),
            );
            Played::Done
        }
        // `Shiv.CreateInHand` for three, then `CardCmd.Upgrade` on each,
        // wherever it landed.
        __c0 if __c0 == V::Potion::CUNNING_POTION => {
            let shiv = CardIdent::new(V::Card::SHIV);
            for at in fight.add_generated_cards(shiv, Pile::Hand, CUNNING_SHIVS) {
                fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
            }
            Played::Done
        }
        // `AnyPlayer`: its Intangible on the player drinking it.
        __c0 if __c0 == V::Potion::GHOST_IN_A_JAR => {
            fight.apply_to_actor(
                V::Power::INTANGIBLE_POWER,
                PowerAmount::Counted(GHOST_IN_A_JAR),
            );
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
