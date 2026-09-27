//! `RunicPyramid`: the hand stays at the end of every turn.

use sts2_core::{DecisionHook, HookResponse};

use super::super::super::V0_107_1 as V;

/// `ShouldFlush`: no for its owner. A relic's owner is the player, and a
/// fight here has one.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldFlush { .. } => HookResponse::Decision(false),
        hook => unimplemented!("what RunicPyramid decides for {hook:?}"),
    }
}
