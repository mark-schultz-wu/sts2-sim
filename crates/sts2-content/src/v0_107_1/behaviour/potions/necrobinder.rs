//! The potions only the Necrobinder's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `BoneBrew`'s `SummonVar(15)`.
const BONE_BREW: Number = 15;

/// `PotionOfDoom`'s `PowerVar<DoomPower>(33)`.
const DOOM: Number = 33;

/// What one of the Necrobinder's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyEnemy`.
        Potion::PotionOfDoom => {
            fight.apply_power(fight.target(), Power::DoomPower, PowerAmount::Counted(DOOM));
            Played::Done
        }
        // `Self`: `OstyCmd.Summon` for its `Summon`.
        Potion::BoneBrew => {
            fight.summon_osty(BONE_BREW);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
