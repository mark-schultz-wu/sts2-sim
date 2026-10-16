//! `LoomingFruit`: more maximum health as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `MaxHpVar(31)`.
const MAX_HP: Number = 31;

/// `AfterObtained`, which is `CreatureCmd.GainMaxHp`. Whether the fruit shows
/// its cornucopia reads the save, and changes its icon alone.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
