//! The Regent's rare cards.

use sts2_core::{CostPool, Fight, Played, PowerAmount};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::{Change, V};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard) -> &'static [Change] {
    match card {
        RegentCard::TheSmith => &[Change::Raise(CardVar::Forge, 10)],
        // `EnergyCost.UpgradeBy(-1)`. The card prints one number, and the
        // upgrade moves the cost rather than that number.
        RegentCard::SwordSage => &[Change::ChangeCost(CostPool::Energy, -1)],
        card => unimplemented!("card upgrade: {card:?}"),
    }
}

/// `TheSmith`. Forges the blade, and does nothing else.
pub(super) fn the_smith<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let forge = fight.var(CardVar::Forge);
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        CardVar::Damage,
        forge,
    );
    Played::Done
}

/// `BigBang`. Draw, fill both of the Regent's resources, then forge the blade.
pub(super) fn big_bang<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.draw(fight.var(CardVar::Cards));
    fight.gain_stars(fight.var(CardVar::Stars));
    fight.modify_energy(fight.var(CardVar::Energy));
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        CardVar::Damage,
        fight.var(CardVar::Forge),
    );
    Played::Done
}

/// `SwordSage`. A power that hands every Sovereign Blade an extra play.
pub(super) fn sword_sage<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.apply_to_actor(
        Power::SwordSagePower,
        PowerAmount::Counted(fight.var(CardVar::SwordSagePower)),
    );
    Played::Done
}
