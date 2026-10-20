//! `RadiantPearl`: a Luminesce in the first hand of every fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `BeforeHandDraw` on its owner's first turn:
/// `AddGeneratedCardsToCombat` of its Luminesces into hand, before the draw.
/// A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => {
            asked.push(HookEffect::GenerateInHand {
                card: V::Card::LUMINESCE,
                count: V::Relic::RADIANT_PEARL_CARDS,
            });
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what RadiantPearl does for {hook:?}"),
    }
}
