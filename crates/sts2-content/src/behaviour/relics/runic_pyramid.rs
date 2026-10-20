//! `RunicPyramid`: the hand stays at the end of every turn.

use sts2_core::{DecisionHook, HookResponse};

/// `ShouldFlush`: no for its owner. A relic's owner is the player, and a
/// fight here has one.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldFlush { .. } => HookResponse::Decision(false),
        hook => unimplemented!("what RunicPyramid decides for {hook:?}"),
    }
}
