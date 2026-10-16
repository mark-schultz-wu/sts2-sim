//! `DataDisk`: Focus in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::generated::encounters::RoomType;
use crate::ids::PowerIds as _;

/// `PowerVar<FocusPower>(1)`.
const FOCUS: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(1);

/// `AfterRoomEntered` in a fight's room: its Focus on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(V::Power::FOCUS_POWER, FOCUS));
    }
}
