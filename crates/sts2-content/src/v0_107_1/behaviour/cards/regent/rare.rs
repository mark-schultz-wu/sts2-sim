//! The Regent's rare cards.

use super::super::super::super::generated::cards::{CardVar, RegentCard};
use super::super::{CardNumber, Number, raise};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard, printed: &mut [CardNumber]) {
    let changes: &[(CardVar, Number)] = match card {
        RegentCard::TheSmith => &[(CardVar::Forge, 10)],
        card => unimplemented!("card upgrade: {card:?}"),
    };
    raise(printed, changes);
}
