//! `ChosenCheese`: a point of Max HP after every fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(1)`.
const MAX_HP: Number = 1;

/// `AfterCombatEnd`: `CreatureCmd.GainMaxHp` on its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CombatEnd { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
