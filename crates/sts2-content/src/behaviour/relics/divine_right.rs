//! `DivineRight`: the stars the Regent opens every fight with.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `StarsVar(3)`, which is the whole of what it gives.
const STARS: Number = 3;

/// `AfterRoomEntered`, which pays the stars on the way into a fight.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::GainStars(STARS));
    }
}
