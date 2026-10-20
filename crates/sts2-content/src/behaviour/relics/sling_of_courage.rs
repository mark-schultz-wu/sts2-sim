//! `SlingOfCourage`: Strength as each elite fight begins.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;
use crate::ids::PowerIds as _;

/// `PowerVar<StrengthPower>(2)`.
const STRENGTH: Number = 2;

/// `AfterRoomEntered` in an elite room: its Strength on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::Elite,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::STRENGTH_POWER,
            PowerAmount::Counted(STRENGTH),
        ));
    }
}
