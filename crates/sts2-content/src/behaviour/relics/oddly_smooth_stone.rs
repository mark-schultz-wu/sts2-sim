//! `OddlySmoothStone`: Dexterity in every fight.

use crate::ids::PowerIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `AfterRoomEntered`: Dexterity on its owner as a fight's room opens.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::DEXTERITY_POWER,
            PowerAmount::Counted(V::Relic::ODDLY_SMOOTH_STONE_DEXTERITY_POWER),
        ));
    }
}
