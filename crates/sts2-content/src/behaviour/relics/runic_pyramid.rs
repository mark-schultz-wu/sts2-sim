//! `RunicPyramid`: the hand stays at the end of every turn.

use sts2_core::{DecisionHook, HookResponse};

/// `ShouldFlush`: no for its owner.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldFlush { player, .. } => HookResponse::Decision(player != owner),
        hook => unimplemented!("what RunicPyramid decides for {hook:?}"),
    }
}
