//! `Strawberry`: more maximum health, once.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(7)`.
const MAX_HP: Number = 7;

/// `AfterObtained`, which is `CreatureCmd.GainMaxHp`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
