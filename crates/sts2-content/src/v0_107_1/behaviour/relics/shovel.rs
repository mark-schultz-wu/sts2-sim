//! `Shovel`: a rest site offers to dig up a relic.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::restsite::RestSiteOption;

/// `TryModifyRestSiteOptions`: `DIG`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::RestSiteOptions => asked.push(HookEffect::OfferRestOption(RestSiteOption::Dig)),
        EventHook::Obtained => {}
        hook => unimplemented!("what Shovel does for {hook:?}"),
    }
}
