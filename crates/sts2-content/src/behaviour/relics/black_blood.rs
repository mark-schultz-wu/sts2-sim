//! `BlackBlood`: Burning Blood refined, a larger heal after every fight won.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `HealVar(12)`.
const HEAL: Number = 12;

/// `AfterCombatVictory`, which heals its owner while its owner is alive. A
/// fight the run wins leaves the player alive.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatVictory { .. } = hook {
        asked.push(HookEffect::Heal(HEAL));
    }
}
