//! `MealTicket`: a heal at each merchant.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `HealVar(15)`.
const HEAL: Number = 15;

/// `AfterRoomEntered`: its heal as a merchant's room opens. Outside a fight
/// its owner is alive.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered {
        room: RoomType::Shop,
        ..
    } = hook
    {
        asked.push(HookEffect::Heal(HEAL));
    }
}
