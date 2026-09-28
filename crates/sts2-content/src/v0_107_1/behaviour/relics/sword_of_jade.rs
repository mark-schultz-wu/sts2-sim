//! `SwordOfJade`: three Strength in every fight, which Sword of Stone turns
//! into.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `PowerVar<StrengthPower>(3)`.
const STRENGTH: PowerAmount<super::Number> = PowerAmount::Counted(3);

/// `AfterRoomEntered`: Strength on its owner as a fight's room opens.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered { room, .. } = hook
        && super::combat_room(room)
    {
        asked.push(HookEffect::ApplyToOwner(Power::StrengthPower, STRENGTH));
    }
}
