//! The Regent's cards.

use sts2_core::Fight;

use super::super::super::generated::cards::{CardVar, RegentCard};
use super::super::super::generated::powers::Power;
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> bool {
    match card {
        RegentCard::Strike => strike(fight),
        RegentCard::Defend => defend(fight),
        RegentCard::FallingStar => falling_star(fight),
        RegentCard::Venerate => venerate(fight),
        _ => return false,
    }
    true
}

/// The card names a target, so it has one: `PlayCardAction` refuses a play
/// `IsValidTarget` rejects, and the game throws here for the same reason.
fn at<F: Fight<V>>(fight: &F) -> u32 {
    fight
        .target()
        .expect("a card that names a target is played at one")
}

/// `StrikeRegent`. Deal damage.
fn strike<F: Fight<V>>(fight: &mut F) {
    let target = at(fight);
    let damage = fight.var(CardVar::Damage);
    fight.attack(target, damage);
}

/// `DefendRegent`. Gain block.
fn defend<F: Fight<V>>(fight: &mut F) {
    let block = fight.var(CardVar::Block);
    fight.gain_block(block);
}

/// `FallingStar`. Deal damage, then weaken and expose what it hit.
///
/// The powers land after the damage, so a target the damage killed has left
/// the fight and takes neither. `PowerCmd.Apply` returns on a creature with no
/// combat state.
fn falling_star<F: Fight<V>>(fight: &mut F) {
    let target = at(fight);
    let damage = fight.var(CardVar::Damage);
    fight.attack(target, damage);
    let weak = fight.var(CardVar::WeakPower);
    fight.apply_power(target, Power::WeakPower, weak);
    let vulnerable = fight.var(CardVar::VulnerablePower);
    fight.apply_power(target, Power::VulnerablePower, vulnerable);
}

/// `Venerate`. Gain stars.
fn venerate<F: Fight<V>>(fight: &mut F) {
    let stars = fight.var(CardVar::Stars);
    fight.gain_stars(stars);
}
