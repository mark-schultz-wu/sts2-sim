//! `MeatOnTheBone`: health back after a fight won at half health or less.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `DynamicVar("HpThreshold", 50)`, a share of the maximum in hundredths.
const THRESHOLD: Number = 50;

/// `HealVar(12)`.
const HEAL: Number = 12;

/// `AfterCombatVictoryEarly`: its heal, when its owner's health is at or
/// below half their maximum. What it does before a fight and as health
/// changes only sets how it looks.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatVictoryEarly { player, .. }
            if Number::from(player.hp) <= Number::from(player.max_hp) * THRESHOLD / 100 =>
        {
            asked.push(HookEffect::Heal(HEAL));
        }
        EventHook::CombatVictoryEarly { .. }
        | EventHook::CombatStarting { .. }
        | EventHook::CurrentHpChanged { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MeatOnTheBone does for {hook:?}"),
    }
}
