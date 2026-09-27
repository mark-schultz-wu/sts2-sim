//! The Ironclad's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardKind, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting, Played,
    Rules as _, Scaling,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, IroncladCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: IroncladCard, fight: &mut F) -> Played<V> {
    match card {
        IroncladCard::Whirlwind => whirlwind(fight),
        card => card_plays::ironclad::on_play(card, fight),
    }
}

/// `Whirlwind`: its damage to every enemy, once for each energy of its X.
fn whirlwind<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let hits = fight.x_value();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            for target in fight.opponents() {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        }
    });
    Played::Done
}

/// Deliver a notification to one Ironclad card in the fight.
pub(super) fn event_hook(
    card: IroncladCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Stomp.BeforeCardPlayed`: one off its cost this turn for each
        // Attack its owner plays, itself included. A fight here has one
        // player.
        (IroncladCard::Stomp, EventHook::CardPlaying { card, .. }) => {
            if V::card_kind(card) == Some(CardKind::Attack) {
                asked.push(HookEffect::ModifySelf {
                    change: Change::ChangeCost(CostPool::Energy, -1),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Stomp.AfterCardEnteredCombat`, for its own entry alone and not a
        // clone's: one off this turn for each Attack play finished this turn
        // before it came.
        (IroncladCard::Stomp, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let attacks: super::Number = history
                    .finished_this_turn()
                    .filter(|&card| V::card_kind(card) == Some(CardKind::Attack))
                    .count()
                    .try_into()
                    .expect("a turn's plays fit a game number");
                if attacks != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(CostPool::Energy, -attacks),
                        lasting: Lasting::ThisTurn,
                    });
                }
            }
        }
        // `DrumOfBattle.AfterCardExhausted`, for its own exhaust alone.
        (IroncladCard::DrumOfBattle, EventHook::CardExhausted { id: exhausted, .. }) => {
            if exhausted == id {
                unimplemented!("a Drum of Battle exhausted, which gains energy for each play")
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
