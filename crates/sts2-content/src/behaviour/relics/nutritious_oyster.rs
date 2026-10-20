//! `NutritiousOyster`: more maximum health as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `CreatureCmd.GainMaxHp` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::NUTRITIOUS_OYSTER_MAX_HP));
    }
}
