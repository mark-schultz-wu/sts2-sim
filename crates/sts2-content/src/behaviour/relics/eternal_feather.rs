//! `EternalFeather`: a heal at each rest site, by the deck's size.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;

/// `AfterRoomEntered` at a rest site: its heal for every whole five cards the
/// deck holds.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RoomEntered {
        room: RoomType::RestSite,
        deck,
        ..
    } = hook
    {
        let fives = Number::try_from(deck / (V::Relic::ETERNAL_FEATHER_CARDS as usize))
            .expect("a deck's size fits a game number");
        asked.push(HookEffect::Heal(V::Relic::ETERNAL_FEATHER_HEAL * fives));
    }
}
