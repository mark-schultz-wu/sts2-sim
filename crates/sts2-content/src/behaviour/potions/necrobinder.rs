//! The potions only the Necrobinder's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use crate::behaviour::potions::Number;
use crate::ids::{CardIds as _, PotionIds as _, PowerIds as _};

/// `PotOfGhouls`'s `CardsVar(2)`.
const POT_OF_GHOULS: usize = 2;

/// `BoneBrew`'s `SummonVar(15)`.
const BONE_BREW: Number = 15;

/// `PotionOfDoom`'s `PowerVar<DoomPower>(33)`.
const DOOM: Number = 33;

/// What one of the Necrobinder's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyEnemy`.
        __c0 if __c0 == V::Potion::POTION_OF_DOOM => {
            fight.apply_power(
                fight.target(),
                V::Power::DOOM_POWER,
                PowerAmount::Counted(DOOM),
            );
            Played::Done
        }
        // `Self`: `OstyCmd.Summon` for its `Summon`.
        __c0 if __c0 == V::Potion::BONE_BREW => {
            fight.summon_osty(BONE_BREW);
            Played::Done
        }
        // `Self`: `Soul.CreateInHand` for its `Cards`.
        __c0 if __c0 == V::Potion::POT_OF_GHOULS => {
            let soul = sts2_core::CardIdent::new(V::Card::SOUL);
            fight.add_generated_cards(soul, sts2_core::Pile::Hand, POT_OF_GHOULS);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
