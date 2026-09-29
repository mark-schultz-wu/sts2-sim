//! `SturdyClamp`: up to ten block kept from turn to turn.

use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `_maxRetainedBlock`.
const KEPT: Number = 10;

/// `ShouldClearBlock`: its owner's block is kept.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldClearBlock { creature, player } => {
            HookResponse::Decision(creature != player)
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// `AfterPreventingBlockClear`, when it was what kept the block: all but
/// ten of it lost. Nothing else reaches it.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::BlockClearPrevented { block } if block > KEPT => {
            asked.push(HookEffect::LoseSomeBlock(block - KEPT));
        }
        EventHook::BlockClearPrevented { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what SturdyClamp does for {hook:?}"),
    }
}
