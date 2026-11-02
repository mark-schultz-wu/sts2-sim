//! `StoneHumidifier`: more maximum health each time the player rests.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterRestSiteHeal`: `CreatureCmd.GainMaxHp` on its `owner`, where the
/// healed player is the owner.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RestSiteHeal { player, .. } = hook
        && player == owner
    {
        asked.push(HookEffect::GainMaxHp(V::Relic::STONE_HUMIDIFIER_MAX_HP));
    }
}
