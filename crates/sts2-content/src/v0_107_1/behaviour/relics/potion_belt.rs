//! `PotionBelt`: two potion slots.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `DynamicVar("PotionSlots", 2)`.
const SLOTS: usize = 2;

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainPotionSlots(SLOTS));
    }
}
