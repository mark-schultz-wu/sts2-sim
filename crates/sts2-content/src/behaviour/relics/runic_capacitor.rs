//! `RunicCapacitor`: three more orb slots as the first turn of a fight
//! starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterSideTurnStart`, for its owner's first turn: `OrbCmd.AddSlots` by
/// three. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn: 1,
        ..
    } = hook
    {
        asked.push(HookEffect::AddOrbSlots(V::Relic::RUNIC_CAPACITOR_REPEAT));
    }
}
