//! `CardModel.OnPlay`, ported.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported.
//!
//! [`on_play`] panics for a card nobody has ported, so this tree is also the
//! list of what the simulator can play. A card outside it stops the run where
//! it stands rather than playing as if it did nothing.

use sts2_core::{CardVars, Fight};

use super::super::V0_107_1;
use super::super::generated::cards::{CardVar, Cards};

/// What the run holds a card's numbers as.
type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

/// One number on a card's face, as this copy of it prints one.
type Printed = sts2_core::Printed<CardVar, Number>;

mod colorless;
mod regent;
mod status;
mod token;

type V = V0_107_1;

/// What a card that works its own target out from the fight names.
pub(crate) fn computed_target(
    card: Cards,
    powers: &[sts2_core::Stack<V0_107_1>],
) -> super::super::generated::cards::TargetType {
    match card {
        Cards::Token(card) => token::computed_target(card, powers),
        card => unimplemented!("what {card:?} points at"),
    }
}

/// What a card works out from the fight for a number it prints.
pub(crate) fn calculated(
    card: Cards,
    key: CardVar,
    printed: &[Printed],
    powers: &[sts2_core::Stack<V0_107_1>],
) -> Number {
    match card {
        Cards::Token(card) => token::calculated(card, key, printed, powers),
        card => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// One of the numbers on a card's own face.
///
/// A key the card does not print is a mistake in the port.
fn face(printed: &[Printed], key: CardVar) -> Number {
    printed
        .iter()
        .find(|var| var.id == key)
        .unwrap_or_else(|| unimplemented!("the {} this card does not print", key.name()))
        .base
}

/// What a card does when it is played.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) {
    match card {
        Cards::Colorless(card) => colorless::on_play(card, fight),
        Cards::Regent(card) => regent::on_play(card, fight),
        Cards::Status(card) => status::on_play(card, fight),
        Cards::Token(card) => token::on_play(card, fight),
        // Nothing from these pools is ported. Named rather than left to a
        // wildcard, so a build that adds a pool stops here.
        Cards::Ironclad(_)
        | Cards::Silent(_)
        | Cards::Necrobinder(_)
        | Cards::Defect(_)
        | Cards::Curse(_)
        | Cards::Event(_)
        | Cards::Quest(_)
        | Cards::Deprecated(_) => unimplemented!("what {card:?} does when it is played"),
    }
}
