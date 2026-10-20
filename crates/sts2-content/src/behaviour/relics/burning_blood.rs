//! `BurningBlood`: the Ironclad's heal after every fight he wins.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCombatVictory`, which heals its owner while its owner is alive. A
/// fight the run wins leaves the player alive.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatVictory { .. } = hook {
        asked.push(HookEffect::Heal(V::Relic::BURNING_BLOOD_HEAL));
    }
}
