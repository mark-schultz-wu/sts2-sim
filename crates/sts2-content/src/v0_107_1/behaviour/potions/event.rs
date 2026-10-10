//! The event pool's potions.

use sts2_core::{Amount, CreatureView as _, Effect, Fight, Played, Scaling};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::TargetType;
use super::super::super::generated::potions::Potion;
use super::Number;

/// `FoulPotion`'s `DamageVar(12, ValueProp.Unpowered)`.
const FOUL: Number = 12;

/// `GlowwaterPotion`'s `CardsVar(10)`.
const GLOWWATER: Number = 10;

/// Where an event potion stopped on the player's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `GlowwaterPotion`, after a question inside its exhausts was answered.
    GlowwaterExhausted,
}

/// The rest of an event potion's use, once the question it stopped on is
/// answered.
pub(super) fn resume<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    answer: sts2_core::Taken<V>,
) -> Played<V> {
    match (at, answer) {
        (Resume::GlowwaterExhausted, sts2_core::Taken::Waited) => {
            fight.draw(GLOWWATER);
            Played::Done
        }
        (at, answer) => unreachable!("{at:?} resumed with {answer:?}"),
    }
}

/// What a potion does when it is drunk in a fight.
///
/// # Panics
///
/// An event potion nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `CreatureCmd.Damage` on every creature in the fight that is not a
        // pet, its drinker included, from the drinker.
        Potion::FoulPotion => {
            let hit: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.alive() && creature.owner().is_none())
                .map(|creature| creature.id())
                .collect();
            for target in hit {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Number(FOUL),
                    scaling: Scaling::Static,
                });
                fight.apply(hit);
            }
            Played::Done
        }
        // `CardCmd.Exhaust` of each card in hand, one at a time, then its
        // `Cards` drawn.
        Potion::GlowwaterPotion => {
            let hand = fight.cards_in(sts2_core::Pile::Hand);
            for card in hand {
                fight.act_on(sts2_core::PileAct::Exhaust, &[card]);
            }
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Potion(super::Resume::Event(
                        Resume::GlowwaterExhausted,
                    )),
                };
            }
            fight.draw(GLOWWATER);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// `TargetType`, for the one potion that reads whether a fight is on:
/// Foul Potion hits every enemy in one, and names no creature outside.
///
/// # Panics
///
/// Any other potion, which prints its target.
pub(super) fn computed_target(potion: Potion, in_combat: bool) -> TargetType {
    match (potion, in_combat) {
        (Potion::FoulPotion, true) => TargetType::AllEnemies,
        (Potion::FoulPotion, false) => TargetType::TargetedNoCreature,
        (potion, _) => unimplemented!("{potion:?} working its target out"),
    }
}

/// `PassesCustomUsabilityCheck`, for Foul Potion: always in a fight, and
/// outside one only at a merchant whose screen is closed. The simulator has
/// no screen, as a headless game has none, so outside a fight it is never
/// usable.
///
/// # Panics
///
/// Any other potion, which answers from its row.
pub(super) fn computed_check(potion: Potion, in_combat: bool) -> bool {
    match potion {
        Potion::FoulPotion => in_combat,
        potion => unimplemented!("{potion:?} checking whether it can be drunk"),
    }
}
