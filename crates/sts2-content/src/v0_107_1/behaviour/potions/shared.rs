//! The potions every character's pool draws from.

use sts2_core::{Fight, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;

/// `DexterityPotion`'s `PowerVar<DexterityPower>(2)`.
const DEXTERITY: i32 = 2;

/// What one of the shared potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) {
    match potion {
        // `AnyPlayer`, which a run with one player points at that player.
        Potion::DexterityPotion => {
            fight.apply_to_actor(Power::DexterityPower, PowerAmount::Counted(DEXTERITY));
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
