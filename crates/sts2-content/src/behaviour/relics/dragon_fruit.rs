//! `DragonFruit`: more maximum health each time its owner gains gold.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterGoldGained` for its owner: `CreatureCmd.GainMaxHp` by one. A relic's
/// owner is the player, and a run here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::GoldGained = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::DRAGON_FRUIT_MAX_HP));
    }
}
