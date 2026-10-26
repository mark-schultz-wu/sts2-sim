//! The potions only the Necrobinder's pool draws.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::{Fight, PowerAmount};

use crate::ids::{CardIds as _, PotionIds as _, PowerIds as _};

/// What one of the Necrobinder's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(
    potion: V::Potion,
    fight: &mut F,
) -> Played<V, F::Kind> {
    match potion {
        // `AnyEnemy`.
        __c0 if __c0 == V::Potion::POTION_OF_DOOM => {
            fight.apply_power(
                fight.target(),
                V::Power::DOOM_POWER,
                PowerAmount::Counted(V::Potion::POTION_OF_DOOM_DOOM_POWER),
            );
            Played::Done
        }
        // `Self`: `OstyCmd.Summon` for its `Summon`.
        __c0 if __c0 == V::Potion::BONE_BREW => {
            fight.summon_osty(V::Potion::BONE_BREW_SUMMON);
            Played::Done
        }
        // `Self`: `Soul.CreateInHand` for its `Cards`.
        __c0 if __c0 == V::Potion::POT_OF_GHOULS => {
            let soul = sts2_core::CardIdent::new(V::Card::SOUL);
            fight.add_generated_cards(
                soul,
                sts2_core::Pile::Hand,
                V::Potion::POT_OF_GHOULS_CARDS as usize,
            );
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
