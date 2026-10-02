//! `NinjaScroll`: three Shivs in the first hand of every fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, TokenCard};
use super::Number;

/// `DynamicVar("Shivs", 3)`.
const SHIVS: Number = 3;

/// `BeforeHandDraw` on its owner's first turn: `Shiv.CreateInHand` of its
/// Shivs, before the draw. A relic's owner is the player, and a fight here
/// has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => {
            asked.push(HookEffect::GenerateInHand {
                card: Cards::Token(TokenCard::Shiv),
                count: SHIVS,
            });
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what NinjaScroll does for {hook:?}"),
    }
}
