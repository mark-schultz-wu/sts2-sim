//! `RingingTriangle`: the hand stays at the end of the first turn.

use sts2_core::{DecisionHook, HookResponse};

/// `ShouldFlush`: only after its owner's first turn. A relic's owner is the
/// player, and a fight here has one.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldFlush { turn, .. } => HookResponse::Decision(turn > 1),
        hook => unimplemented!("what RingingTriangle decides for {hook:?}"),
    }
}
