//! The Regent's uncommon cards.

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::{CardNumber, Number, V, raise};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard, printed: &mut [CardNumber]) {
    let changes: &[(CardVar, Number)] = match card {
        RegentCard::Bulwark => &[(CardVar::Block, 3), (CardVar::Forge, 3)],
        card => unimplemented!("card upgrade: {card:?}"),
    };
    raise(printed, changes);
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
