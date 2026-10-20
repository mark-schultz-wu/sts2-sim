//! `NutritiousOyster`: more maximum health as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `MaxHpVar(11)`.
const MAX_HP: Number = 11;

/// `AfterObtained`: `CreatureCmd.GainMaxHp` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
