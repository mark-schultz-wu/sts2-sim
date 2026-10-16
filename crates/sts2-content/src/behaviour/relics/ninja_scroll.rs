//! `NinjaScroll`: three Shivs in the first hand of every fight.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `DynamicVar("Shivs", 3)`.
const SHIVS: Number = 3;

/// `BeforeHandDraw` on its owner's first turn: `Shiv.CreateInHand` of its
/// Shivs, before the draw. A relic's owner is the player, and a fight here
/// has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => {
            asked.push(HookEffect::GenerateInHand {
                card: V::Card::SHIV,
                count: SHIVS,
            });
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what NinjaScroll does for {hook:?}"),
    }
}
