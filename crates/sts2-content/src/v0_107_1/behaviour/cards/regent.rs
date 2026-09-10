//! The Regent's cards.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::powers::Power;

use super::super::super::generated::cards::{CardVar, Cards, RegentCard};
use super::{V, printed};

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) {
    // Carried down, so a card reads the numbers it was played as rather than
    // the ones whichever card its rule was written beside prints.
    let id = Cards::Regent(card);
    match card {
        RegentCard::Strike => strike(id, fight),
        RegentCard::Defend => defend(id, fight),
        RegentCard::FallingStar => falling_star(id, fight),
        RegentCard::GuidingStar => guiding_star(id, fight),
        RegentCard::Venerate => venerate(id, fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `StrikeRegent`. Deal damage.
fn strike<F: Fight<V>>(card: Cards, fight: &mut F) {
    let target = fight.target();
    fight.attack(target, printed(card, CardVar::Damage));
}

/// `DefendRegent`. Gain block.
fn defend<F: Fight<V>>(card: Cards, fight: &mut F) {
    fight.gain_block(printed(card, CardVar::Block));
}

/// `FallingStar`. Deal damage, then weaken and expose what it hit.
///
/// The powers land after the damage, so a target the damage killed has left
/// the fight and takes neither. `PowerCmd.Apply` returns on a creature with no
/// combat state.
fn falling_star<F: Fight<V>>(card: Cards, fight: &mut F) {
    let target = fight.target();
    fight.attack(target, printed(card, CardVar::Damage));
    fight.apply(
        target,
        Power::WeakPower,
        PowerAmount::Counted(printed(card, CardVar::WeakPower)),
    );
    fight.apply(
        target,
        Power::VulnerablePower,
        PowerAmount::Counted(printed(card, CardVar::VulnerablePower)),
    );
}

/// `GuidingStar`. Deal damage, then draw.
fn guiding_star<F: Fight<V>>(card: Cards, fight: &mut F) {
    let target = fight.target();
    fight.attack(target, printed(card, CardVar::Damage));
    fight.draw(printed(card, CardVar::Cards));
}

/// `Venerate`. Gain stars.
fn venerate<F: Fight<V>>(card: Cards, fight: &mut F) {
    fight.gain_stars(printed(card, CardVar::Stars));
}
