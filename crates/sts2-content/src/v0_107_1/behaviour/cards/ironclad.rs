//! The Ironclad's cards the generator leaves to a hand port.

use sts2_core::{CardKind, Change, CostPool, EventHook, HookEffect, Lasting, Rules as _};

use super::super::super::generated::cards::IroncladCard;
use super::V;

/// Deliver a notification to one Ironclad card in the fight.
pub(super) fn event_hook(
    card: IroncladCard,
    id: sts2_core::CardId<V>,
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
        // `Stomp.AfterCardEnteredCombat`, for its own entry alone: one off
        // for each Attack played this turn before it came.
        (IroncladCard::Stomp, EventHook::CardEnteredCombat { at, .. }) => {
            if at.id() == id {
                unimplemented!("a Stomp entering the fight, which counts the turn's Attacks")
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
