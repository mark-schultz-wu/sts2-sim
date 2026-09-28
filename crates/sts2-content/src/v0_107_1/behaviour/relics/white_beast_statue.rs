//! `WhiteBeastStatue`: a potion after every fight won.

use sts2_core::{DecisionHook, EventHook, HookResponse};

use super::super::super::V0_107_1 as V;

/// `ShouldForcePotionReward`: for a fight's room. A run here has one
/// player, who owns it.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldForcePotionReward { room } => {
            HookResponse::Decision(super::combat_room(room))
        }
        _ => HookResponse::DoesNotImplement,
    }
}

pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained => {}
        hook => unimplemented!("what WhiteBeastStatue does for {hook:?}"),
    }
}
