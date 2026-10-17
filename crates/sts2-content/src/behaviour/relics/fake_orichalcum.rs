//! `FakeOrichalcum`: block at a turn's end that has none.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `ShouldTrigger`, which it keeps for the length of a turn's end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    should_trigger: bool,
}

/// `BeforeSideTurnEndVeryEarly` arms it where its owner's side ends a turn
/// with no block. `BeforeSideTurnEnd` then spends that on its block, and
/// `BeforeSideTurnStart` on its owner's side disarms it.
pub(super) fn event_hook<V: crate::Build>(
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
            asked.push(HookEffect::GainBlock(V::Relic::FAKE_ORICHALCUM_BLOCK));
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.should_trigger = false,
        _ => {}
    }
}
