//! The Regent's rare cards.

use sts2_core::{Fight, Played};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::{Change, V};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard) -> &'static [Change] {
    match card {
        RegentCard::TheSmith => &[Change::Raise(CardVar::Forge, 10)],
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
