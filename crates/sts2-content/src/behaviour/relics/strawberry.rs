//! `Strawberry`: more maximum health, once.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`, which is `CreatureCmd.GainMaxHp`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::STRAWBERRY_MAX_HP));
    }
}
