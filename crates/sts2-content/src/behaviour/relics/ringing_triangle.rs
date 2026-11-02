//! `RingingTriangle`: the hand stays at the end of the first turn.

use sts2_core::{DecisionHook, HookResponse};

/// `ShouldFlush`: only after its owner's first turn.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldFlush { player, turn } => {
            HookResponse::Decision(player != owner || turn > 1)
        }
        hook => unimplemented!("what RingingTriangle decides for {hook:?}"),
    }
}
