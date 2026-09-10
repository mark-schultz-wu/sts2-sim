//! `CardModel.OnPlay`, ported.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported.
//!
//! [`on_play`] panics for a card nobody has ported, so this tree is also the
//! list of what the simulator can play. A card outside it stops the run where
//! it stands rather than playing as if it did nothing.

use sts2_core::Fight;

use super::super::V0_107_1;
use super::super::generated::cards::Cards;

mod regent;

type V = V0_107_1;

/// What a card does when it is played.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) {
    match card {
        Cards::Regent(card) => regent::on_play(card, fight),
        // Nothing from these pools is ported. Named rather than left to a
        // wildcard, so a build that adds a pool stops here.
        Cards::Ironclad(_)
        | Cards::Silent(_)
        | Cards::Necrobinder(_)
        | Cards::Defect(_)
        | Cards::Colorless(_)
        | Cards::Curse(_)
        | Cards::Event(_)
        | Cards::Quest(_)
        | Cards::Status(_)
        | Cards::Token(_)
        | Cards::Deprecated(_) => unimplemented!("what {card:?} does when it is played"),
    }
}
