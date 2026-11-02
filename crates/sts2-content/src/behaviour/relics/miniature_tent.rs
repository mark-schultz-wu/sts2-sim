//! `MiniatureTent`: a rest site's other options stay up once one goes
//! through.

use sts2_core::{DecisionHook, EventHook, HookResponse};

/// `ShouldDisableRemainingRestSiteOptions`: no, for its owner.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldDisableRemainingRestSiteOptions { player } => {
            HookResponse::Decision(player != owner)
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MiniatureTent does for {hook:?}"),
    }
}
