//! The potions only the Defect's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `FocusPotion`'s `PowerVar<FocusPower>(2)`.
const FOCUS: Number = 2;

/// `PotionOfCapacity`'s `RepeatVar(2)`: the orb slots it adds.
const CAPACITY: Number = 2;

/// What one of the Defect's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // On its drinker, whom `Self` names.
        Potion::FocusPotion => {
            fight.apply_to_actor(Power::FocusPower, PowerAmount::Counted(FOCUS));
            Played::Done
        }
        // `OrbCmd.AddSlots` for its drinker.
        Potion::PotionOfCapacity => {
            fight.add_orb_slots(CAPACITY);
            Played::Done
        }
        // `OrbCmd.Channel<DarkOrb>` once for each of its drinker's orb slots.
        Potion::EssenceOfDarkness => {
            for _ in 0..fight.orb_capacity() {
                fight.channel(super::super::super::generated::orbs::Orb::DarkOrb);
            }
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
