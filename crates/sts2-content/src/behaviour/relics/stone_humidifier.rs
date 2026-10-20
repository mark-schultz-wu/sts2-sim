//! `StoneHumidifier`: more maximum health each time the player rests.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterRestSiteHeal`: `CreatureCmd.GainMaxHp` on its owner, the only player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RestSiteHeal { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::STONE_HUMIDIFIER_MAX_HP));
    }
}
