//! The token pool's potions, which only a relic or a card makes.

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use crate::behaviour::potions::Number;
use crate::ids::PotionIds as _;

/// `PotionShapedRock`'s `DamageVar(15, ValueProp.Unpowered)`.
const ROCK: Number = 15;

/// What a potion does when it is drunk in a fight.
///
/// # Panics
///
/// A token potion nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `CreatureCmd.Damage` on the target, unpowered and from the player.
        __c0 if __c0 == V::Potion::POTION_SHAPED_ROCK => {
            let target = fight.target();
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Number(ROCK),
                scaling: Scaling::Static,
            });
            fight.apply(hit);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
