//! `FakeMango`: a little more maximum health, once.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `MaxHpVar(3)`.
const MAX_HP: Number = 3;

/// `AfterObtained`, which is `CreatureCmd.GainMaxHp`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
