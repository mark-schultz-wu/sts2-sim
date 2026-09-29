//! `CaptainsWheel`: block as the third turn's block clears.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `BlockVar(18, ValueProp.Unpowered)`.
const BLOCK: Number = 18;

/// `AfterBlockCleared` for its owner on turn three: `CreatureCmd.GainBlock`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
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
