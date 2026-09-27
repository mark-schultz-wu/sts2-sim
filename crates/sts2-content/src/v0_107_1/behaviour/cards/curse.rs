//! The curses the generator leaves to a hand port.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::CurseCard;
use super::super::super::generated::powers::Power;
use super::V;

/// `Shame`'s `DynamicVar("Frail", 1)`.
const SHAME_FRAIL: super::Number = 1;

/// A curse's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<F: Fight<V>>(card: CurseCard, fight: &mut F) {
    match card {
        CurseCard::Shame => shame(fight),
        card => card_plays::curse::on_turn_end(card, fight),
    }
}

/// `Shame`: its Frail onto its holder, which then goes to the discard pile.
/// Frail landing on the player skips its next tick, which is what Shame sets
/// on a fresh one.
fn shame<F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(Power::FrailPower, PowerAmount::Counted(SHAME_FRAIL));
    fight.discard_this();
}
