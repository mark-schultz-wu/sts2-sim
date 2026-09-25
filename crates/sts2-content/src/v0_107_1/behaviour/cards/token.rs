//! The cards a fight makes, which no deck holds.

use sts2_core::{Amount, CardVars, Effect, Fight, Played, Scaling};

use super::super::super::generated::card_plays;
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
        card => unimplemented!("what {card:?} points at"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: TokenCard, fight: &mut F) -> Played<V> {
    match card {
        TokenCard::SovereignBlade => sovereign_blade(fight),
        card => card_plays::token::on_play(card, fight),
    }
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
