//! `BloodVial`: two points of health back on the first turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStartLate`: its heal on its owner's first turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStartLate { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::PlayerTurnStartLate { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::Heal(V::Relic::BLOOD_VIAL_HEAL));
    }
}
