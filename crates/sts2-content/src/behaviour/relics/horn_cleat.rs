//! `HornCleat`: block as the second turn begins.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BlockVar(14, Unpowered)`.
const BLOCK: Number = 14;

/// `AfterBlockCleared`: its block once its owner's block clears on the second
/// turn.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BlockCleared {
        creature,
        player,
        turn,
    } = hook
        && creature == player
        && turn == 2
    {
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
