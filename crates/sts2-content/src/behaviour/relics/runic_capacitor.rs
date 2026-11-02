//! `RunicCapacitor`: three more orb slots as the first turn of a fight
//! starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnStart`, for its owner's first turn: `OrbCmd.AddSlots` by
/// three. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        participants,
        turn: 1,
        ..
    } = hook
        && participants.contains(owner)
    {
        asked.push(HookEffect::AddOrbSlots(V::Relic::RUNIC_CAPACITOR_REPEAT));
    }
}
