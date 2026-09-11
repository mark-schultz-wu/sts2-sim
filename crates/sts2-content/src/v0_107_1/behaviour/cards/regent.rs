//! The Regent's cards.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::powers::Power;

use super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) {
    match card {
        RegentCard::Strike => strike(fight),
        RegentCard::Defend => defend(fight),
        RegentCard::FallingStar => falling_star(fight),
        RegentCard::GuidingStar => guiding_star(fight),
        RegentCard::Venerate => venerate(fight),
        RegentCard::RefineBlade => refine_blade(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `StrikeRegent`. Deal damage.
fn strike<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(target, fight.var(CardVar::Damage));
}

/// `DefendRegent`. Gain block.
fn defend<F: Fight<V>>(fight: &mut F) {
    fight.gain_block(fight.var(CardVar::Block));
}

/// `FallingStar`. Deal damage, then weaken and expose what it hit.
///
/// The powers land after the damage, so a target the damage killed has left
/// the fight and takes neither. `PowerCmd.Apply` returns on a creature with no
/// combat state.
fn falling_star<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(target, fight.var(CardVar::Damage));
    fight.apply(
        target,
        Power::WeakPower,
        PowerAmount::Counted(fight.var(CardVar::WeakPower)),
    );
    fight.apply(
        target,
        Power::VulnerablePower,
        PowerAmount::Counted(fight.var(CardVar::VulnerablePower)),
    );
}

/// `GuidingStar`. Deal damage, then draw.
fn guiding_star<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(target, fight.var(CardVar::Damage));
    fight.draw(fight.var(CardVar::Cards));
}

/// `Venerate`. Gain stars.
fn venerate<F: Fight<V>>(fight: &mut F) {
    fight.gain_stars(fight.var(CardVar::Stars));
}

/// `RefineBlade`. Forge every Sovereign Blade, then gain energy next turn.
///
/// `ForgeCmd.Forge` creates a blade when the fight holds none it can reach,
/// and raises the damage on every one it holds. A blade in the exhaust pile is
/// raised and does not count as one it can reach.
fn refine_blade<F: Fight<V>>(fight: &mut F) {
    let forge = fight.var(CardVar::Forge);
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        CardVar::Damage,
        forge,
    );
    fight.apply_to_actor(
        Power::EnergyNextTurnPower,
        PowerAmount::Counted(fight.var(CardVar::Energy)),
    );
}
