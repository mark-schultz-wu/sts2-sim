//! `LeesWaffle`: more maximum health, and the whole of it back, once.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `CreatureCmd.GainMaxHp`, then `CreatureCmd.Heal` for what
/// its owner's health then lacks.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::LEES_WAFFLE_MAX_HP));
        asked.push(HookEffect::HealMissing);
    }
}
