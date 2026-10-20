//! `MeatOnTheBone`: health back after a fight won at half health or less.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `AfterCombatVictoryEarly`: its heal, when its owner's health is at or
/// below half their maximum. What it does before a fight and as health
/// changes only sets how it looks.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatVictoryEarly { player, .. }
            if Number::from(player.hp)
                <= Number::from(player.max_hp) * V::Relic::MEAT_ON_THE_BONE_HP_THRESHOLD / 100 =>
        {
            asked.push(HookEffect::Heal(V::Relic::MEAT_ON_THE_BONE_HEAL));
        }
        EventHook::CombatVictoryEarly { .. }
        | EventHook::CombatStarting { .. }
        | EventHook::CurrentHpChanged { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MeatOnTheBone does for {hook:?}"),
    }
}
