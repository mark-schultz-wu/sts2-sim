//! `WhiteBeastStatue`: a potion after every fight won.

use sts2_core::{DecisionHook, EventHook, HookResponse};

/// `ShouldForcePotionReward`: for a fight's room. A run here has one
/// player, who owns it.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldForcePotionReward { room } => {
            HookResponse::Decision(crate::behaviour::relics::combat_room(room))
        }
        _ => HookResponse::DoesNotImplement,
    }
}

pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what WhiteBeastStatue does for {hook:?}"),
    }
}
