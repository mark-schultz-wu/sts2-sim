//! `OddlySmoothStone`: Dexterity in every fight.

use crate::ids::PowerIds as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `PowerVar<DexterityPower>(1)`.
const DEXTERITY: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(1);

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
            DEXTERITY,
        ));
    }
}
