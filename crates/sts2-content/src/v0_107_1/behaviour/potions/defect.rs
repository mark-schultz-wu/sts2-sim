//! The potions only the Defect's pool draws.

use sts2_core::{Fight, Played, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `FocusPotion`'s `PowerVar<FocusPower>(2)`.
const FOCUS: Number = 2;

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
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
