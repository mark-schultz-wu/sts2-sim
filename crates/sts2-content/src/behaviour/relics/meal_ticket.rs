//! `MealTicket`: a heal at each merchant.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

/// `AfterRoomEntered`: its heal as a merchant's room opens. Outside a fight
/// its owner is alive.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::Shop,
        ..
    } = hook
    {
        asked.push(HookEffect::Heal(V::Relic::MEAL_TICKET_HEAL));
    }
}
