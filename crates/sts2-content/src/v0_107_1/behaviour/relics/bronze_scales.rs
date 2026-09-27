//! `BronzeScales`: Thorns in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::powers::Power;

/// `PowerVar<ThornsPower>(3)`.
const THORNS: PowerAmount<super::Number> = PowerAmount::Counted(3);

/// `AfterRoomEntered`: Thorns on its owner as a fight's room opens.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered {
        room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(Power::ThornsPower, THORNS));
    }
}
