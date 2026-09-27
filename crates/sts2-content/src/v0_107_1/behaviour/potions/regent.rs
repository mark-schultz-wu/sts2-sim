//! The potions only the Regent's pool draws.

use sts2_core::{Fight, Played};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;
use super::Number;

/// `StarPotion`'s `StarsVar(3)`.
const STARS: Number = 3;

/// What one of the Regent's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `PlayerCmd.GainStars` by its `Stars`.
        Potion::StarPotion => {
            fight.gain_stars(STARS);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
