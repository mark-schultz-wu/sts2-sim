//! `Orichalcum`: block at a turn's end that has none.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `BlockVar(6, Unpowered)`.
const BLOCK: Number = 6;

/// `ShouldTrigger`, which it keeps for the length of a turn's end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    should_trigger: bool,
}

/// `BeforeSideTurnEndVeryEarly` arms it where its owner's side ends a turn
/// with no block. `BeforeSideTurnEnd` then spends that on its block, and
/// `BeforeSideTurnStart` on its owner's side disarms it.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeSideTurnEndVeryEarly {
            side: CombatSide::Player,
            block,
        } if block <= 0 => state.should_trigger = true,
        EventHook::BeforeSideTurnEnd { .. } if state.should_trigger => {
            state.should_trigger = false;
            asked.push(HookEffect::GainBlock(BLOCK));
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.should_trigger = false,
        _ => {}
    }
}
