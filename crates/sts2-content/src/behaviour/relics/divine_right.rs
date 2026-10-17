//! `DivineRight`: the stars the Regent opens every fight with.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterRoomEntered`, which pays the stars on the way into a fight.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::GainStars(V::Relic::DIVINE_RIGHT_STARS));
    }
}
