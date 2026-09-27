//! The potions only the Silent's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PoisonPotion`'s `PowerVar<PoisonPower>(6)`.
const POISON: Number = 6;

/// What one of the Silent's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyEnemy`.
        Potion::PoisonPotion => {
            fight.apply_power(
                fight.target(),
                Power::PoisonPower,
                PowerAmount::Counted(POISON),
            );
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
