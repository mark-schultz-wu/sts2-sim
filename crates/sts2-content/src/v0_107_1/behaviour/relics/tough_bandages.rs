//! `ToughBandages`: block for each card its owner discards in its own turn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `BlockVar(3, ValueProp.Unpowered)`.
const BLOCK: Number = 3;

/// `AfterCardDiscarded` in its owner's side's turn: its block on its owner.
/// A fight here has one player, whose every card is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardDiscarded {
            side: CombatSide::Player,
        } => asked.push(HookEffect::GainBlock(BLOCK)),
        EventHook::CardDiscarded { .. } | EventHook::Obtained => {}
        hook => unimplemented!("what ToughBandages does for {hook:?}"),
    }
}
