//! `SturdyClamp`: up to ten block kept from turn to turn.

use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse};

use crate::behaviour::relics::Number;

/// `_maxRetainedBlock`.
const KEPT: Number = 10;

/// `ShouldClearBlock`: its owner's block is kept.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldClearBlock { creature } => HookResponse::Decision(creature != owner),
        _ => HookResponse::DoesNotImplement,
    }
}

/// `AfterPreventingBlockClear`, when it was what kept the block: all but
/// ten of it lost. Nothing else reaches it.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BlockClearPrevented { block } if block > KEPT => {
            asked.push(HookEffect::LoseSomeBlock(block - KEPT));
        }
        EventHook::BlockClearPrevented { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what SturdyClamp does for {hook:?}"),
    }
}
