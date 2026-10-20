//! The potions only the Ironclad's pool draws.

use crate::vars::PotionVars as _;
use sts2_core::{CreatureView as _, Fight, Pile, PileAct, Played};

use crate::behaviour::potions::Number;
use crate::ids::PotionIds as _;

/// Where one of the Ironclad's potions stops to ask something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Ashwater`, after the cards picked to exhaust.
    Ashwater,
}

/// Carry an Ironclad potion on with the cards picked.
pub(super) fn resume<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    picked: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.Exhaust` on each, one at a time.
        Resume::Ashwater => {
            fight.act_on(PileAct::Exhaust, picked.as_slice());
            Played::Done
        }
    }
}

/// What one of the Ironclad's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyPlayer`: a fifth of its maximum, which `SetCurrentHpInternal`
        // truncates.
        __c0 if __c0 == V::Potion::BLOOD_POTION => {
            let actor = fight.actor();
            let max_hp: Number = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map(|creature| Number::from(creature.max_hp()))
                .expect("the player drinking stands in the fight");
            fight.heal(actor, max_hp * V::Potion::BLOOD_POTION_HEAL_PERCENT / 100);
            Played::Done
        }
        // `Self`: any number of cards of the hand, each exhausted.
        __c0 if __c0 == V::Potion::ASHWATER => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::UpTo(
                nonmax::NonMaxUsize::new(crate::behaviour::potions::ANY_NUMBER)
                    .expect("a count below the maximum"),
            ),
            at: crate::behaviour::potions::Resume::Ironclad(Resume::Ashwater).into(),
        },
        // `AnyPlayer`: one more replay on every card of the player's in the
        // fight tagged Strike.
        __c0 if __c0 == V::Potion::SOLDIERS_STEW => {
            use crate::generated::cards::CardTag;

            fight.add_replays(sts2_core::Writing::EveryTagged(CardTag::Strike), 1);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
