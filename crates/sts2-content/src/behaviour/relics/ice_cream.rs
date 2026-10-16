//! `IceCream`: energy left at a turn's end carries into the next.

use sts2_core::{DecisionHook, EventHook, HookResponse};

/// `ShouldPlayerResetEnergy`: only on the first turn. A fight here has one
/// player, who owns it.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldPlayerResetEnergy { turn, .. } => HookResponse::Decision(turn == 1),
        _ => HookResponse::DoesNotImplement,
    }
}

pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what IceCream does for {hook:?}"),
    }
}
