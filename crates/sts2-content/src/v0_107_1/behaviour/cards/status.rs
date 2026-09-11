//! The cards a fight puts in a deck that nobody chose.

use sts2_core::Fight;

use super::super::super::generated::cards::{CardVar, StatusCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    match card {
        StatusCard::Slimed => slimed(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `Slimed`. Draws, for an energy, and exhausts.
fn slimed<F: Fight<V>>(fight: &mut F) {
    fight.draw(fight.var(CardVar::Cards));
}
