//! `LeesWaffle`: more maximum health, and the whole of it back, once.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(7)`.
const MAX_HP: Number = 7;

/// `AfterObtained`: `CreatureCmd.GainMaxHp`, then `CreatureCmd.Heal` for what
/// its owner's health then lacks.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
        asked.push(HookEffect::HealMissing);
    }
}
