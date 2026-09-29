//! `MiniatureTent`: a rest site's other options stay up once one goes
//! through.

use sts2_core::{DecisionHook, EventHook, HookResponse};

use super::super::super::V0_107_1 as V;

/// `ShouldDisableRemainingRestSiteOptions`: no, for its owner.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldDisableRemainingRestSiteOptions => HookResponse::Decision(false),
        _ => HookResponse::DoesNotImplement,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MiniatureTent does for {hook:?}"),
    }
}
