//! The cards a fight puts in a deck that nobody chose.

use sts2_core::{Amount, Effect, EventHook, Fight, HookEffect, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, StatusCard};
use super::V;

/// A status card's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    match card {
        StatusCard::Infection | StatusCard::Burn | StatusCard::Wither | StatusCard::Toxic => {
            infection(fight)
        }
        card => card_plays::status::on_turn_end(card, fight),
    }
}

/// `Infection`, `Burn`, `Wither` and `Toxic`, each of which hurts its holder
/// by its damage and goes to the discard pile.
fn infection<F: Fight<V>>(fight: &mut F) {
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
