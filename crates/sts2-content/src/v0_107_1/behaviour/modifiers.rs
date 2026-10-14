//! What a run modifier does, as ported code. A modifier is asked only about
//! a hook its class overrides.

use sts2_core::{DecisionHook, HookResponse};

use super::super::V0_107_1 as V;
use super::super::generated::modifiers::Modifier;

/// What `modifier` decides for `hook`.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn decide_hook(modifier: Modifier, hook: DecisionHook<V>) -> HookResponse<bool> {
    match (modifier, hook) {
        // `Flight.ShouldAllowFreeTravel`.
        (Modifier::Flight, DecisionHook::ShouldAllowFreeTravel) => HookResponse::Decision(true),
        (modifier, hook) => unimplemented!("what {modifier:?} decides for {hook:?}"),
    }
}

/// Whether every rule of `modifier` is ported.
pub(crate) fn ported(modifier: Modifier) -> bool {
    matches!(modifier, Modifier::Flight)
}
