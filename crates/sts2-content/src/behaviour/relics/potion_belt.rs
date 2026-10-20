//! `PotionBelt`: two potion slots.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainPotionSlots(
            V::Relic::POTION_BELT_POTION_SLOTS as usize,
        ));
    }
}
