//! `BoundPhylactery`: the Necrobinder's Osty, summoned each turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeCombatStart`, and `AfterEnergyResetLate` on every turn after the
/// first. Both are `OstyCmd.Summon` for its owner, the only player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatStarting { .. } => {
            asked.push(HookEffect::Summon(V::Relic::BOUND_PHYLACTERY_SUMMON))
        }
        EventHook::EnergyResetLate { turn, .. } if turn != 1 => {
            asked.push(HookEffect::Summon(V::Relic::BOUND_PHYLACTERY_SUMMON));
        }
        _ => {}
    }
}
