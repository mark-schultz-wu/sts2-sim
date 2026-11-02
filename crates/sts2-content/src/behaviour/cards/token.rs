//! The cards a fight makes, which no deck holds.

use crate::behaviour::Played;
use sts2_core::{
    Amount, CardVars, CreatureView as _, Effect, Fight, FightKind, Opponents, Scaling,
};

use crate::behaviour::cards::Number;
use crate::behaviour::paused::{Ask, Lent};
use crate::generated::cards::TargetType;
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// What a card works out from the fight for a number it prints.
pub(super) fn calculated<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    key: V::CardVar,
    printed: &P,
    powers: &[sts2_core::Stack<V>],
) -> Number {
    match (card, key) {
        // `CalculatedVar.Calculate`: a base, plus an extra for each of
        // whatever the card counts. Sovereign Blade counts the player's Parry,
        // and prints no block without one.
        (__c0, __c1)
            if (__c0 == V::Card::SOVEREIGN_BLADE && __c1 == V::CardVar::CALCULATED_BLOCK) =>
        {
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * parry::<V>(powers)
        }
        (card, key) => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// `GetOwnerParryAmount`, which is the block a Parry hands a card.
fn parry<V: crate::Build>(powers: &[sts2_core::Stack<V>]) -> Number {
    let Some(held) = powers
        .iter()
        .find(|held| held.power == V::Power::PARRY_POWER)
    else {
        return 0;
    };
    match held.amount {
        sts2_core::PowerAmount::Counted(amount) => amount,
        sts2_core::PowerAmount::Endless => unimplemented!("an endless Parry"),
    }
}

/// What a card that works its own target out from the fight names.
pub(super) fn computed_target<V: crate::Build>(
    card: V::Card,
    carries: impl Fn(V::Power) -> bool,
) -> TargetType {
    match card {
        // `SovereignBlade.TargetType`: one enemy, or the lot under Seeking
        // Edge.
        __c0 if __c0 == V::Card::SOVEREIGN_BLADE => match carries(V::Power::SEEKING_EDGE_POWER) {
            true => TargetType::AllEnemies,
            false => TargetType::AnyEnemy,
        },
        // `Shiv.TargetType`: the same, under Fan of Knives.
        __c0 if __c0 == <V::Card as crate::ids::CardIds>::SHIV => {
            match carries(V::Power::FAN_OF_KNIVES_POWER) {
                true => TargetType::AllEnemies,
                false => TargetType::AnyEnemy,
            }
        }
        card => unimplemented!("what {card:?} points at"),
    }
}

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        __c0 if __c0 == V::Card::SOVEREIGN_BLADE => start::<V, K>(card, fight),
        __c0 if __c0 == <V::Card as crate::ids::CardIds>::SHIV => shiv::<V, _>(fight),
        __c0 if __c0 == V::Card::SWEEPING_GAZE => sweeping_gaze::<V, _>(fight),
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// `SweepingGaze`: where Osty stands, its damage from Osty to an enemy drawn
/// at random.
fn sweeping_gaze<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        let Some(target) = fight.random_target() else {
            return;
        };
        let hit = fight.resolve(Effect::Attack {
            dealer: osty,
            target,
            damage: Amount::Var(V::CardVar::OSTY_DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(hit);
    });
    Played::Done
}

/// The creatures a card's attack hits: every opponent while the player
/// holds `power`, and otherwise the one the card was pointed at.
///
/// `TargetingAllOpponents` takes the list again for each hit.
fn targets<V: crate::Build, F: Fight<V>>(fight: &F, power: V::Power) -> Opponents {
    let actor = fight.actor();
    let widened = fight
        .creatures()
        .any(|creature| creature.id() == actor && creature.power(power).is_some());
    match widened {
        true => fight.opponents(),
        false => [fight.target()].into_iter().collect(),
    }
}

/// `Shiv`. Deal its damage once, to every opponent under Fan of Knives.
fn shiv<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = targets::<V, _>(fight, V::Power::FAN_OF_KNIVES_POWER);
        // One `CreatureCmd.Damage` over them all.
        fight.damage_command(|fight| {
            for target in targets {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(V::CardVar::DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        });
    });
    Played::Done
}

/// A token card's play that can stop on a question: from its start to its
/// end, yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `SovereignBlade`. Deal its damage, as many times as it repeats, to
        // every opponent under Seeking Edge. Then, while its owner holds
        // Parry, gain its `CalculatedBlock`.
        //
        // The damage is this copy's own. A forge raises it, and the card
        // that was forged twice hits for more than the one beside it.
        __c0 if __c0 == V::Card::SOVEREIGN_BLADE => {
            sovereign_blade::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            sovereign_blade_hit::<V, _>(resumed!().fight);
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Card, crate::behaviour::cards::Resume::Token);

/// `SovereignBlade`'s attack: its damage, as many times as it repeats, to
/// every opponent under Seeking Edge.
fn sovereign_blade<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..fight.var(V::CardVar::REPEAT) {
            let targets = targets::<V, _>(fight, V::Power::SEEKING_EDGE_POWER);
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(V::CardVar::DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
        }
    });
}

/// The rest of `SovereignBlade`, once its attack is over: its block where its
/// owner carries Parry.
fn sovereign_blade_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    // `GetOwnerParryAmount(this) > 0`.
    let actor = fight.actor();
    let parried = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .and_then(|creature| creature.power(V::Power::PARRY_POWER))
        .is_some_and(|amount| match amount {
            sts2_core::PowerAmount::Counted(amount) => amount > 0,
            sts2_core::PowerAmount::Endless => unimplemented!("an endless Parry"),
        });
    if parried {
        let block = fight.resolve(Effect::GainBlock {
            recipient: actor,
            amount: Amount::Var(V::CardVar::CALCULATED_BLOCK),
            scaling: Scaling::Dynamic,
        });
        fight.apply(block);
    }
}
