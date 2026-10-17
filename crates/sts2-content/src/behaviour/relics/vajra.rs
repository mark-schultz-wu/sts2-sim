//! `Vajra`: Strength in every fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::generated::encounters::RoomType;
use crate::ids::PowerIds as _;

/// `AfterRoomEntered`: Strength on its owner as a fight's room opens.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::STRENGTH_POWER,
            PowerAmount::Counted(V::Relic::VAJRA_STRENGTH_POWER),
        ));
    }
}
