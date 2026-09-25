//! The cards a fight puts in a deck that nobody chose.

use sts2_core::{Amount, Effect, EventHook, Fight, HookEffect, Played, Scaling};

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

/// Deliver a notification to one status card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: StatusCard,
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `Void.AfterCardDrawn`, which acts on the draw that brought it here and
    // on no other card's.
    match (card, hook) {
        (StatusCard::Void, EventHook::CardDrawn { id: drawn, .. }) => {
            if drawn == id {
                asked.push(HookEffect::LoseEnergy(printed.face(CardVar::Energy)));
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// `Slimed`. Draws, for an energy, and exhausts.
fn slimed<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.draw(fight.var(CardVar::Cards));
    Played::Done
}
