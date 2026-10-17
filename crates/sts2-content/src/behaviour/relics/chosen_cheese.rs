//! `ChosenCheese`: a point of Max HP after every fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCombatEnd`: `CreatureCmd.GainMaxHp` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatEnd { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::CHOSEN_CHEESE_MAX_HP));
    }
}
