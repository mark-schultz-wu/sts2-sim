//! `DataDisk`: Focus in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::powers::Power;

/// `PowerVar<FocusPower>(1)`.
const FOCUS: PowerAmount<super::Number> = PowerAmount::Counted(1);

/// `AfterRoomEntered` in a fight's room: its Focus on its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered {
        room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(Power::FocusPower, FOCUS));
    }
}
