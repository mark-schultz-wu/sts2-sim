//! `Mango`: more maximum health as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`, which is `CreatureCmd.GainMaxHp`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::MANGO_MAX_HP));
    }
}
