//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::{Change, CostPool, EventHook, HookEffect, Lasting};

use super::super::super::generated::cards::{CardVar, NecrobinderCard};
use super::super::super::generated::monsters::Monster;
use super::V;

/// Deliver a notification to one Necrobinder card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: NecrobinderCard,
    printed: &P,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Flatten.AfterAttack`, which answers an attack by its owner's Osty
        // alone. A fight here has one player, so any Osty is theirs.
        (
            NecrobinderCard::Flatten,
            EventHook::AfterAttack {
                monster: Some(Monster::Osty),
                ..
            },
        ) => unimplemented!("Flatten's cost after an Osty attack"),
        (NecrobinderCard::Flatten, EventHook::AfterAttack { .. }) => {}
        // `Melancholy.AfterDeath`: any creature's death takes its `Energy` off
        // its cost for the rest of the fight. The sim prevents no death, so
        // every one counts, and a card that hears a hook is in a combat pile.
        (NecrobinderCard::Melancholy, EventHook::AfterDeath { .. }) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(CardVar::Energy)),
                lasting: Lasting::ThisFight,
            })
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
