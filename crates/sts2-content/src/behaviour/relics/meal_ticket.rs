//! `MealTicket`: a heal at each merchant.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;

/// `HealVar(15)`.
const HEAL: Number = 15;

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
        asked.push(HookEffect::Heal(HEAL));
    }
}
