//! `SwordOfJade`: three Strength in every fight, which Sword of Stone turns
//! into.

use crate::ids::PowerIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `AfterRoomEntered`: Strength on its owner as a fight's room opens.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::STRENGTH_POWER,
            PowerAmount::Counted(V::Relic::SWORD_OF_JADE_STRENGTH_POWER),
        ));
    }
}
