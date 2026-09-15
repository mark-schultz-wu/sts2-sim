//! The potions every character's pool draws from.

use sts2_core::{Fight, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `DexterityPotion`'s `PowerVar<DexterityPower>(2)`.
const DEXTERITY: Number = 2;

/// `VulnerablePotion`'s `PowerVar<VulnerablePower>(3)`.
const VULNERABLE: Number = 3;

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
        Potion::VulnerablePotion => {
            fight.apply_power(
                fight.target(),
                Power::VulnerablePower,
                PowerAmount::Counted(VULNERABLE),
            );
        }
        // `AnyPlayer` again, and a run with one player upgrades that player's
        // hand.
        Potion::BlessingOfTheForge => fight.upgrade_hand(),
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
