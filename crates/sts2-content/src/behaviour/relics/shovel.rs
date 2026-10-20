//! `Shovel`: a rest site offers to dig up a relic.

use crate::ids::RestSiteOptionIds as _;
use sts2_core::{EventHook, HookEffect};

/// `TryModifyRestSiteOptions`: `DIG`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RestSiteOptions => {
            asked.push(HookEffect::OfferRestOption(V::RestSiteOption::DIG))
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Shovel does for {hook:?}"),
    }
}
