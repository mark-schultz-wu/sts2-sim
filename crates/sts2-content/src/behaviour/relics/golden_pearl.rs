//! `GoldenPearl`: gold, once, as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`, which is `PlayerCmd.GainGold`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainGold(V::Relic::GOLDEN_PEARL_GOLD));
    }
}
