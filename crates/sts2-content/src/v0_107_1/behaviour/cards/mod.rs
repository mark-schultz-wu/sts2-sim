//! `CardModel.OnPlay`, ported.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported.
//!
//! [`on_play`] panics for a card nobody has ported, so this tree is also the
//! list of what the simulator can play. A card outside it stops the run where
//! it stands rather than playing as if it did nothing.

use sts2_core::{CardTable, Fight};

use super::super::V0_107_1;
use super::super::generated::cards::{CardVar, Cards};

mod colorless;
mod regent;
mod status;

type V = V0_107_1;

/// One of the numbers a card prints, as the run holds numbers.
///
/// Static: a card's face does not depend on the fight it is played in. A value
/// the fight has had its say over reaches a card through [`Fight`].
///
/// A key the card does not print is a mistake in the port. So is a printed
/// number no game number can hold.
fn printed(card: Cards, key: CardVar) -> <V as sts2_core::ContentVersion>::Number {
    card.vars()
        .iter()
        .find(|var| var.id == key)
        .and_then(|var| num_traits::NumCast::from(var.base))
        .expect("a card reads a number it prints")
}

/// What a card does when it is played.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) {
    match card {
        Cards::Colorless(card) => colorless::on_play(card, fight),
        Cards::Regent(card) => regent::on_play(card, fight),
        Cards::Status(card) => status::on_play(card, fight),
        // Nothing from these pools is ported. Named rather than left to a
        // wildcard, so a build that adds a pool stops here.
        Cards::Ironclad(_)
        | Cards::Silent(_)
        | Cards::Necrobinder(_)
        | Cards::Defect(_)
        | Cards::Curse(_)
        | Cards::Event(_)
        | Cards::Quest(_)
        | Cards::Token(_)
        | Cards::Deprecated(_) => unimplemented!("what {card:?} does when it is played"),
    }
}
