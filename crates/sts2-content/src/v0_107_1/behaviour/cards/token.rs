//! The cards a fight makes, which no deck holds.

use sts2_core::{CardVars, Effect, Fight};

use super::super::super::generated::cards::{CardVar, TargetType, TokenCard};
use super::super::super::generated::powers::Power;
use super::{CardNumber, Number, V, face};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: TokenCard, printed: &mut [CardNumber]) {
    let changes: &[(CardVar, Number)] = match card {
        TokenCard::MinionStrike => &[(CardVar::Damage, 3)],
        card => unimplemented!("card upgrade: {card:?}"),
    };
    super::raise(printed, changes);
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

pub(super) fn on_play<F: Fight<V>>(card: TokenCard, fight: &mut F) {
    match card {
        TokenCard::SovereignBlade => sovereign_blade(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `SovereignBlade`. Deal its damage, as many times as it repeats.
///
/// The damage is this copy's own. A forge raises it, and the card that was
/// forged twice hits for more than the one beside it.
fn sovereign_blade<F: Fight<V>>(fight: &mut F) {
    let damage = fight.var(CardVar::Damage);
    let target = fight.target();
    for _ in 0..fight.var(CardVar::Repeat) {
        let attack = fight.resolve(Effect::Attack { target, damage });
        fight.apply(attack);
    }
}
