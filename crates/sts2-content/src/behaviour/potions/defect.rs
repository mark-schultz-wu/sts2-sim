//! The potions only the Defect's pool draws.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::{Fight, PowerAmount};

use crate::ids::{OrbIds as _, PotionIds as _, PowerIds as _};

/// What one of the Defect's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(
    potion: V::Potion,
    fight: &mut F,
) -> Played<V, F::Kind> {
    match potion {
        // On its drinker, whom `Self` names.
        __c0 if __c0 == V::Potion::FOCUS_POTION => {
            fight.apply_to_actor(
                V::Power::FOCUS_POWER,
                PowerAmount::Counted(V::Potion::FOCUS_POTION_FOCUS_POWER),
            );
            Played::Done
        }
        // `OrbCmd.AddSlots` for its drinker.
        __c0 if __c0 == V::Potion::POTION_OF_CAPACITY => {
            fight.add_orb_slots(V::Potion::POTION_OF_CAPACITY_REPEAT);
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
