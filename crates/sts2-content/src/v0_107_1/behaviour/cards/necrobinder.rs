//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::EventHook;

use super::super::super::generated::cards::NecrobinderCard;
use super::super::super::generated::monsters::Monster;
use super::V;

/// Deliver a notification to one Necrobinder card in the fight.
pub(super) fn event_hook(
    card: NecrobinderCard,
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
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
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
