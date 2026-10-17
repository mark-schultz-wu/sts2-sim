//! `Gorget`: Plating as every fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::ids::PowerIds as _;

/// `AfterRoomEntered`, which applies Plating to its owner in a combat room.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::PLATING_POWER,
            PowerAmount::Counted(V::Relic::GORGET_PLATING_POWER),
        ));
    }
}
