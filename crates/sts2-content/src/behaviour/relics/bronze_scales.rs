//! `BronzeScales`: Thorns in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::generated::encounters::RoomType;
use crate::ids::PowerIds as _;

/// `PowerVar<ThornsPower>(3)`.
const THORNS: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(3);

/// `AfterRoomEntered`: Thorns on its owner as a fight's room opens.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(V::Power::THORNS_POWER, THORNS));
    }
}
