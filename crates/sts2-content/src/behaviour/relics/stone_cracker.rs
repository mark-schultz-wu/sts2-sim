//! `StoneCracker`: two cards of the draw pile upgraded as every fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterRoomEntered` for a fight's room: two of the draw pile's upgradable
/// cards, stably shuffled off `CombatCardSelection`, upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered { room, .. } = hook
        && crate::behaviour::relics::combat_room(room)
    {
        asked.push(HookEffect::UpgradeRandomDrawCards(
            V::Relic::STONE_CRACKER_CARDS as usize,
        ));
    }
}
