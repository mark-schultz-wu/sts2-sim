//! `ChosenCheese`: a point of Max HP after every fight.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `MaxHpVar(1)`.
const MAX_HP: Number = 1;

/// `AfterCombatEnd`: `CreatureCmd.GainMaxHp` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatEnd { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
