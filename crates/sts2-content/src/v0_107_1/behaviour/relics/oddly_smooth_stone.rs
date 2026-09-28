//! `OddlySmoothStone`: Dexterity in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `PowerVar<DexterityPower>(1)`.
const DEXTERITY: PowerAmount<super::Number> = PowerAmount::Counted(1);

/// `AfterRoomEntered`: Dexterity on its owner as a fight's room opens.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered { room, .. } = hook
        && super::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(Power::DexterityPower, DEXTERITY));
    }
}
