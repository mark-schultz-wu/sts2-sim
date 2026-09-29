//! `IceCream`: energy left at a turn's end carries into the next.

use sts2_core::{DecisionHook, EventHook, HookResponse};

use super::super::super::V0_107_1 as V;

/// `ShouldPlayerResetEnergy`: only on the first turn. A fight here has one
/// player, who owns it.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldPlayerResetEnergy { turn, .. } => HookResponse::Decision(turn == 1),
        _ => HookResponse::DoesNotImplement,
    }
}

pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what IceCream does for {hook:?}"),
    }
}
