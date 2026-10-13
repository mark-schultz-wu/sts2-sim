//! The cards a fight puts in a deck that nobody chose.

use sts2_core::{Amount, Effect, EventHook, Fight, HookEffect, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, StatusCard};
use super::super::super::generated::powers::Power;
use super::V;

/// What a status card does when it is played.
pub(super) fn on_play<F: Fight<V>>(card: StatusCard, fight: &mut F) -> sts2_core::Played<V> {
    match card {
        StatusCard::FranticEscape => frantic_escape(fight),
        card => card_plays::status::on_play(card, fight),
    }
}

/// `FranticEscape`: one more on the Sandpit of the first enemy that holds
/// one, then its own cost one higher for the rest of the fight. A fight here
/// has one player, whom every Sandpit targets.
fn frantic_escape<F: Fight<V>>(fight: &mut F) -> sts2_core::Played<V> {
    use sts2_core::CreatureView as _;

    let player_side = fight.side();
    let pit = fight
        .creatures()
        .find(|creature| {
            creature.side() != player_side && creature.power(Power::SandpitPower).is_some()
        })
        .map(|creature| creature.id());
    if let Some(pit) = pit {
        fight.modify_power_amount(pit, Power::SandpitPower, 1, Some(pit));
    }
    fight.modify_this(
        sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, 1),
        sts2_core::Lasting::ThisFight,
    );
    sts2_core::Played::Done
}

/// A status card's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<F: Fight<V>>(card: StatusCard, fight: &mut F) {
    match card {
        StatusCard::Infection | StatusCard::Burn | StatusCard::Wither | StatusCard::Toxic => {
            infection(fight)
        }
        // `Beckon`: its `HpLoss` off its holder, unblockable, and then it goes
        // to the discard pile.
        StatusCard::Beckon => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            fight.discard_after_turn_end();
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
    fight.discard_after_turn_end();
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
