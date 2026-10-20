//! `CaptainsWheel`: block as the third turn's block clears.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BlockVar(18, ValueProp.Unpowered)`.
const BLOCK: Number = 18;

/// `AfterBlockCleared` for its owner on turn three: `CreatureCmd.GainBlock`.
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
        && turn == 3
    {
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
