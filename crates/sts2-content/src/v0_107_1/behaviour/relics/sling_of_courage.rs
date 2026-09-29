//! `SlingOfCourage`: Strength as each elite fight begins.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<StrengthPower>(2)`.
const STRENGTH: Number = 2;

/// `AfterRoomEntered` in an elite room: its Strength on its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered {
        room: RoomType::Elite,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(
            Power::StrengthPower,
            PowerAmount::Counted(STRENGTH),
        ));
    }
}
