//! The cards a fight puts in a deck that nobody chose.

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::super::generated::cards::{CardVar, StatusCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: StatusCard, fight: &mut F) -> Played<V> {
    match card {
        StatusCard::Slimed => slimed(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// A status card's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    if card != StatusCard::Infection {
        return;
    }
    let damage = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target: fight.actor(),
        damage: Amount::Var(CardVar::Damage),
        scaling: Scaling::Static,
    });
    fight.apply(damage);
    fight.discard_this();
}

/// A status card's effect as it is drawn.
pub(super) fn on_drawn<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    if card == StatusCard::Void {
        fight.lose_energy(fight.var(CardVar::Energy));
    }
}

/// `Slimed`. Draws, for an energy, and exhausts.
fn slimed<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.draw(fight.var(CardVar::Cards));
    Played::Done
}
