//! The Regent's basic cards.

use sts2_core::{Amount, Effect, Fight, PowerAmount};

use super::super::super::super::generated::cards::{CardVar, RegentCard};
use super::super::super::super::generated::powers::Power;
use super::super::{CardNumber, Number, V, raise};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard, printed: &mut [CardNumber]) {
    let changes: &[(CardVar, Number)] = match card {
        RegentCard::Strike => &[(CardVar::Damage, 3)],
        RegentCard::Defend => &[(CardVar::Block, 3)],
        card => unimplemented!("card upgrade: {card:?}"),
    };
    raise(printed, changes);
}

/// `StrikeRegent`. Deal damage.
pub(super) fn strike<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Printed(CardVar::Damage),
    });
    fight.apply(attack);
}

/// `DefendRegent`. Gain block.
pub(super) fn defend<F: Fight<V>>(fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Printed(CardVar::Block),
    });
    fight.apply(block);
}

/// `FallingStar`. Deal damage, then weaken and expose what it hit.
///
/// The powers land after the damage, so a target the damage killed has left
/// the fight and takes neither. `PowerCmd.Apply` returns on a creature with no
/// combat state.
pub(super) fn falling_star<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Printed(CardVar::Damage),
    });
    fight.apply(attack);
    fight.apply_power(
        target,
        Power::WeakPower,
        PowerAmount::Counted(fight.var(CardVar::WeakPower)),
    );
    fight.apply_power(
        target,
        Power::VulnerablePower,
        PowerAmount::Counted(fight.var(CardVar::VulnerablePower)),
    );
}

/// `Venerate`. Gain stars.
pub(super) fn venerate<F: Fight<V>>(fight: &mut F) {
    fight.gain_stars(fight.var(CardVar::Stars));
}
