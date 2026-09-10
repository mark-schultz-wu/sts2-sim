//! The cards a fight puts in a deck that nobody chose.

use sts2_core::Fight;

use super::super::super::generated::cards::{CardVar, Cards, StatusCard};
use super::{V, printed};

pub(super) fn on_play<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    let id = Cards::Status(card);
    match card {
        StatusCard::Slimed => slimed(id, fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `Slimed`. Draws, for an energy, and exhausts.
fn slimed<F: Fight<V>>(card: Cards, fight: &mut F) {
    fight.draw(printed(card, CardVar::Cards));
}
