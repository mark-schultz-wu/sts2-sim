//! `DragonFruit`: more maximum health each time its owner gains gold.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(1)`.
const MAX_HP: Number = 1;

/// `AfterGoldGained` for its owner: `CreatureCmd.GainMaxHp` by one. A relic's
/// owner is the player, and a run here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::GoldGained = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
