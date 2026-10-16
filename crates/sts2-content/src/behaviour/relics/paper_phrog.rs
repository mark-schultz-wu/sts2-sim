//! `PaperPhrog`: Vulnerable takes more from its owner's hits. Its
//! `ModifyVulnerableMultiplier` is no hook, and `VulnerablePower` reads it off
//! the dealer, so it overrides nothing.

use sts2_core::EventHook;

pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what PaperPhrog does for {hook:?}"),
    }
}
