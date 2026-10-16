//! `StoneHumidifier`: more maximum health each time the player rests.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `MaxHpVar(5)`.
const MAX_HP: Number = 5;

/// `AfterRestSiteHeal`: `CreatureCmd.GainMaxHp` on its owner, the only player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RestSiteHeal { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}
