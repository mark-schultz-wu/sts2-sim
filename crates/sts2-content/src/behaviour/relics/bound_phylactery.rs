//! `BoundPhylactery`: the Necrobinder's Osty, summoned each turn.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `SummonVar(1)`.
const SUMMON: Number = 1;

/// `BeforeCombatStart`, and `AfterEnergyResetLate` on every turn after the
/// first. Both are `OstyCmd.Summon` for its owner, the only player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatStarting { .. } => asked.push(HookEffect::Summon(SUMMON)),
        EventHook::EnergyResetLate { turn, .. } if turn != 1 => {
            asked.push(HookEffect::Summon(SUMMON));
        }
        _ => {}
    }
}
