//! The cards a fight makes, which no deck holds.

use sts2_core::{Amount, CardVars, CreatureView as _, Effect, Fight, Opponents, Played, Scaling};

use super::super::super::generated::cards::Cards;
use super::super::super::generated::cards::{CardVar, TargetType, TokenCard};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// What a card works out from the fight for a number it prints.
pub(super) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: TokenCard,
    key: CardVar,
    printed: &P,
    powers: &[sts2_core::Stack<V>],
) -> Number {
    match (card, key) {
        // `CalculatedVar.Calculate`: a base, plus an extra for each of
        // whatever the card counts. Sovereign Blade counts the player's Parry,
        // and prints no block without one.
        (TokenCard::SovereignBlade, CardVar::CalculatedBlock) => {
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * parry(powers)
        }
        (card, key) => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// `GetOwnerParryAmount`, which is the block a Parry hands a card.
fn parry(powers: &[sts2_core::Stack<V>]) -> Number {
    let Some(held) = powers.iter().find(|held| held.power == Power::ParryPower) else {
        return 0;
    };
    match held.amount {
        sts2_core::PowerAmount::Counted(amount) => amount,
        sts2_core::PowerAmount::Endless => unimplemented!("an endless Parry"),
    }
}

/// What a card that works its own target out from the fight names.
pub(super) fn computed_target(card: TokenCard, powers: &[sts2_core::Stack<V>]) -> TargetType {
    match card {
        // `SovereignBlade.TargetType`: one enemy, or the lot under Seeking
        // Edge.
        TokenCard::SovereignBlade => {
            match powers.iter().any(|h| h.power == Power::SeekingEdgePower) {
                true => TargetType::AllEnemies,
                false => TargetType::AnyEnemy,
            }
        }
        // `Shiv.TargetType`: the same, under Fan of Knives.
        TokenCard::Shiv => match powers.iter().any(|h| h.power == Power::FanOfKnivesPower) {
            true => TargetType::AllEnemies,
            false => TargetType::AnyEnemy,
        },
        card => unimplemented!("what {card:?} points at"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: TokenCard, fight: &mut F) -> Played<V> {
    match card {
        TokenCard::SovereignBlade => sovereign_blade(fight),
        TokenCard::Shiv => shiv(fight),
        TokenCard::SweepingGaze => sweeping_gaze(fight),
        card => super::generated_from(Cards::Token(card), fight, 0),
    }
}

/// `SweepingGaze`: where Osty stands, its damage from Osty to an enemy drawn
/// at random.
fn sweeping_gaze<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
            damage: Amount::Var(CardVar::OstyDamage),
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
fn targets<F: Fight<V>>(fight: &F, power: Power) -> Opponents {
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
fn shiv<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = targets(fight, Power::FanOfKnivesPower);
        // One `CreatureCmd.Damage` over them all.
        fight.damage_command(|fight| {
            for target in targets {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        });
    });
    Played::Done
}

/// `SovereignBlade`. Deal its damage, as many times as it repeats, to every
/// opponent under Seeking Edge. Then, while its owner holds Parry, gain its
/// `CalculatedBlock`.
///
/// The damage is this copy's own. A forge raises it, and the card that was
/// forged twice hits for more than the one beside it.
fn sovereign_blade<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..fight.var(CardVar::Repeat) {
            let targets = targets(fight, Power::SeekingEdgePower);
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
        }
    });
    // `GetOwnerParryAmount(this) > 0`.
    let actor = fight.actor();
    let parried = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .and_then(|creature| creature.power(Power::ParryPower))
        .is_some_and(|amount| match amount {
            sts2_core::PowerAmount::Counted(amount) => amount > 0,
            sts2_core::PowerAmount::Endless => unimplemented!("an endless Parry"),
        });
    if parried {
        let block = fight.resolve(Effect::GainBlock {
            recipient: actor,
            amount: Amount::Var(CardVar::CalculatedBlock),
            scaling: Scaling::Dynamic,
        });
        fight.apply(block);
    }
    Played::Done
}
