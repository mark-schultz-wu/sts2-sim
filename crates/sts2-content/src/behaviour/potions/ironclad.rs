//! The potions only the Ironclad's pool draws.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::{CreatureView as _, Fight, Pile, PileAct};

use crate::behaviour::potions::Number;
use crate::ids::PotionIds as _;

/// What one of the Ironclad's potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    potion: V::Potion,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
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
        __c0 if __c0 == V::Potion::ASHWATER => start::<V, K>(potion, fight),
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

/// One of the Ironclad's potions that stops on a question: from its use to
/// its end, yielding each question it stops on. Each step is lent the fight,
/// and after a question the answer.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: sts2_core::FightKind<V>>(potion: V::Potion) {
    match potion {
        // `Ashwater`, `Self`: any number of cards of the hand, then
        // `CardCmd.Exhaust` on each, one at a time.
        __c0 if __c0 == V::Potion::ASHWATER => {
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::UpTo(
                    nonmax::NonMaxUsize::new(crate::behaviour::potions::ANY_NUMBER)
                        .expect("a count below the maximum"),
                ),
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::Exhaust,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
        }
        potion => unreachable!("{potion:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Potion, crate::behaviour::potions::Resume::Ironclad);
