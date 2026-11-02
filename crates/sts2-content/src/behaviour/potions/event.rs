//! The event pool's potions.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::{Amount, CreatureView as _, Effect, Fight, Scaling};

use crate::generated::cards::TargetType;
use crate::ids::PotionIds as _;

/// What a potion does when it is drunk in a fight.
///
/// # Panics
///
/// An event potion nobody has ported.
pub(super) fn on_use<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    potion: V::Potion,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match potion {
        // `CreatureCmd.Damage` on every creature in the fight that is not a
        // pet, its drinker included, from the drinker.
        __c0 if __c0 == V::Potion::FOUL_POTION => {
            let hit: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.alive() && creature.owner().is_none())
                .map(|creature| creature.id())
                .collect();
            let targets = hit;
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Number(V::Potion::FOUL_POTION_DAMAGE),
                        scaling: Scaling::Static,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        __c0 if __c0 == V::Potion::GLOWWATER_POTION => start::<V, K>(potion, fight),
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// An event potion that stops on a question: from its use to its end,
/// yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: sts2_core::FightKind<V>>(potion: V::Potion) {
    match potion {
        // `GlowwaterPotion`: `CardCmd.Exhaust` of each card in hand, one at
        // a time, then its `Cards` drawn. A question inside the exhausts
        // stands the draw still.
        __c0 if __c0 == V::Potion::GLOWWATER_POTION => {
            let fight = &mut *resumed!().fight;
            for card in fight.cards_in(sts2_core::Pile::Hand) {
                fight.act_on(sts2_core::PileAct::Exhaust, &[card]);
            }
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            resumed!().fight.draw(V::Potion::GLOWWATER_POTION_CARDS);
        }
        potion => unreachable!("{potion:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Potion, crate::behaviour::potions::Resume::Event);

/// `TargetType`, for the one potion that reads whether a fight is on:
/// Foul Potion hits every enemy in one, and names no creature outside.
///
/// # Panics
///
/// Any other potion, which prints its target.
pub(super) fn computed_target<V: crate::Build>(potion: V::Potion, in_combat: bool) -> TargetType {
    match (potion, in_combat) {
        (__c0, true) if __c0 == V::Potion::FOUL_POTION => TargetType::AllEnemies,
        (__c0, false) if __c0 == V::Potion::FOUL_POTION => TargetType::TargetedNoCreature,
        (potion, _) => unimplemented!("{potion:?} working its target out"),
    }
}

/// `PassesCustomUsabilityCheck`, for Foul Potion: always in a fight, and
/// outside one only where `GetFoulPotionMerchantTarget` finds a merchant's
/// button, which `merchant` says.
///
/// # Panics
///
/// Any other potion, which answers from its row.
pub(super) fn computed_check<V: crate::Build>(
    potion: V::Potion,
    in_combat: bool,
    merchant: bool,
) -> bool {
    match potion {
        __c0 if __c0 == V::Potion::FOUL_POTION => in_combat || merchant,
        potion => unimplemented!("{potion:?} checking whether it can be drunk"),
    }
}
