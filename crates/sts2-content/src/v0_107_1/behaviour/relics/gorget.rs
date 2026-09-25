//! `Gorget`: Plating as every fight opens.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<PlatingPower>(4)`.
const PLATING: Number = 4;

/// `AfterRoomEntered`, which applies Plating to its owner in a combat room.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered { room, .. } = hook
        && super::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(
            Power::PlatingPower,
            PowerAmount::Counted(PLATING),
        ));
    }
}
