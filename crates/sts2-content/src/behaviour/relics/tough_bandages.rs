//! `ToughBandages`: block for each card its owner discards in its own turn.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `BlockVar(3, ValueProp.Unpowered)`.
const BLOCK: Number = 3;

/// `AfterCardDiscarded` in its owner's side's turn: its block on its owner.
/// A fight here has one player, whose every card is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDiscarded {
            side: CombatSide::Player,
        } => asked.push(HookEffect::GainBlock(BLOCK)),
        EventHook::CardDiscarded { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ToughBandages does for {hook:?}"),
    }
}
