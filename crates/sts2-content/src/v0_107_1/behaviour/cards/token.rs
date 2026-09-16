//! The cards a fight makes, which no deck holds.

use sts2_core::{Amount, CardVars, CostPool, Effect, Fight, Played, Scaling};

use super::super::super::generated::cards::{CardVar, TargetType, TokenCard};
use super::super::super::generated::powers::Power;
use super::{CardNumber, Change, Number, V, face};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: TokenCard) -> &'static [Change] {
    match card {
        TokenCard::MinionStrike => &[Change::Raise(CardVar::Damage, 3)],
        // `SovereignBlade.OnUpgrade` is `EnergyCost.UpgradeBy(-1)`, and it
        // raises nothing the card prints.
        TokenCard::SovereignBlade => &[Change::ChangeCost(CostPool::Energy, -1)],
        card => unimplemented!("card upgrade: {card:?}"),
    }
}

/// What a card works out from the fight for a number it prints.
pub(super) fn calculated(
    card: TokenCard,
    key: CardVar,
    printed: &[CardNumber],
    powers: &[sts2_core::Stack<V>],
) -> Number {
    match (card, key) {
        // `CalculatedVar.Calculate`: a base, plus an extra for each of
        // whatever the card counts. Sovereign Blade counts the player's Parry,
        // and prints no block without one.
        (TokenCard::SovereignBlade, CardVar::CalculatedBlock) => {
            face(printed, CardVar::CalculationBase)
                + face(printed, CardVar::CalculationExtra) * parry(powers)
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
        card => unimplemented!("what {card:?} points at"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: TokenCard, fight: &mut F) -> Played<V> {
    match card {
        TokenCard::MinionStrike => minion_strike(fight),
        TokenCard::SovereignBlade => sovereign_blade(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `MinionStrike`. Deal damage, then draw.
fn minion_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Var(CardVar::Damage),
        scaling: Scaling::Dynamic,
    });
    fight.apply(attack);
    fight.draw(fight.var(CardVar::Cards));
    Played::Done
}

/// `SovereignBlade`. Deal its damage, as many times as it repeats.
///
/// The damage is this copy's own. A forge raises it, and the card that was
/// forged twice hits for more than the one beside it.
fn sovereign_blade<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    for _ in 0..fight.var(CardVar::Repeat) {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    }
    Played::Done
}
