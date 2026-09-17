//! The Regent's uncommon cards.

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::{Change, V};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard) -> &'static [Change] {
    match card {
        RegentCard::Furnace => &[Change::Raise(CardVar::Forge, 2)],
        RegentCard::Bulwark => &[
            Change::Raise(CardVar::Block, 3),
            Change::Raise(CardVar::Forge, 3),
        ],
        card => unimplemented!("card upgrade: {card:?}"),
    }
}

/// `Bulwark`. Block, then forge the blade.
pub(super) fn bulwark<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let forge = fight.var(CardVar::Forge);
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        CardVar::Damage,
        forge,
    );
    Played::Done
}

/// `Furnace`. Apply a forge at the start of each turn.
pub(super) fn furnace<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = fight.var(CardVar::Forge);
    fight.apply_to_actor(
        super::super::super::super::generated::powers::Power::FurnacePower,
        sts2_core::PowerAmount::Counted(amount),
    );
    Played::Done
}
