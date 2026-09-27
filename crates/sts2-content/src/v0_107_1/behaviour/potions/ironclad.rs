//! The potions only the Ironclad's pool draws.

use sts2_core::{CreatureView as _, Fight, Played};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::Number;

/// `BloodPotion`'s `DynamicVar("HealPercent", 20)`.
const HEAL_PERCENT: Number = 20;

/// What one of the Ironclad's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyPlayer`: a fifth of its maximum, which `SetCurrentHpInternal`
        // truncates.
        Potion::BloodPotion => {
            let actor = fight.actor();
            let max_hp: Number = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map(|creature| Number::from(creature.max_hp()))
                .expect("the player drinking stands in the fight");
            fight.heal(actor, max_hp * HEAL_PERCENT / 100);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
