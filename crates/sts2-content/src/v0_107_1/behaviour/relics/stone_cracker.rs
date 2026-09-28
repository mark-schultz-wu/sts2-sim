//! `StoneCracker`: two cards of the draw pile upgraded as every fight opens.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `AfterRoomEntered` for a fight's room: two of the draw pile's upgradable
/// cards, stably shuffled off `CombatCardSelection`, upgraded.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RoomEntered { room, .. } = hook
        && super::combat_room(room)
    {
        asked.push(HookEffect::UpgradeRandomDrawCards(CARDS));
    }
}
