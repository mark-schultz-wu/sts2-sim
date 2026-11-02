//! `Orichalcum`: block at a turn's end that has none.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `ShouldTrigger`, which it keeps for the length of a turn's end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    should_trigger: bool,
}

/// `BeforeSideTurnEndVeryEarly` arms it where its owner's side ends a turn
/// with no block. `BeforeSideTurnEnd` then spends that on its block, and
/// `BeforeSideTurnStart` on its owner's side disarms it.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeSideTurnEndVeryEarly {
            participants,
            block,
            ..
        } if participants.contains(owner) && block <= 0 => state.should_trigger = true,
        EventHook::BeforeSideTurnEnd { .. } if state.should_trigger => {
            state.should_trigger = false;
            asked.push(HookEffect::GainBlock(V::Relic::ORICHALCUM_BLOCK));
        }
        EventHook::BeforeSideTurnStart { participants, .. } if participants.contains(owner) => {
            state.should_trigger = false
        }
        _ => {}
    }
}
