//! `BurningBlood`: the Ironclad's heal after every fight he wins.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `HealVar(6)`.
const HEAL: Number = 6;

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
