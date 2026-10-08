//! `PaperKrane`: Weak takes more off hits on its owner. Its
//! `ModifyWeakMultiplier` is no hook, and `WeakPower` reads it off the
//! target, so it overrides nothing.

use sts2_core::EventHook;

use super::super::super::V0_107_1 as V;

pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what PaperKrane does for {hook:?}"),
    }
}
