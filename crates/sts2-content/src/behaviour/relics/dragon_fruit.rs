//! `DragonFruit`: more maximum health each time its owner gains gold.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterGoldGained` for its owner: `CreatureCmd.GainMaxHp` by one.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::GoldGained { player } = hook
        && player == owner
    {
        asked.push(HookEffect::GainMaxHp(V::Relic::DRAGON_FRUIT_MAX_HP));
    }
}
