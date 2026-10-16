//! The potions only the Defect's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use crate::behaviour::potions::Number;
use crate::ids::{OrbIds as _, PotionIds as _, PowerIds as _};

/// `FocusPotion`'s `PowerVar<FocusPower>(2)`.
const FOCUS: Number = 2;

/// `PotionOfCapacity`'s `RepeatVar(2)`: the orb slots it adds.
const CAPACITY: Number = 2;

/// What one of the Defect's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // On its drinker, whom `Self` names.
        __c0 if __c0 == V::Potion::FOCUS_POTION => {
            fight.apply_to_actor(V::Power::FOCUS_POWER, PowerAmount::Counted(FOCUS));
            Played::Done
        }
        // `OrbCmd.AddSlots` for its drinker.
        __c0 if __c0 == V::Potion::POTION_OF_CAPACITY => {
            fight.add_orb_slots(CAPACITY);
            Played::Done
        }
        // `OrbCmd.Channel<DarkOrb>` once for each of its drinker's orb slots.
        __c0 if __c0 == V::Potion::ESSENCE_OF_DARKNESS => {
            for _ in 0..fight.orb_capacity() {
                fight.channel(V::Orb::DARK_ORB);
            }
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
