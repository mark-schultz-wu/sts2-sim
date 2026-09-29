//! `ThrowingAxe`: the first card of each fight plays twice.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `UsedThisCombat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used: bool,
}

/// `AfterRoomEntered` for a fight's room and `AfterCombatEnd` both make it
/// ready. `AfterModifyingCardPlayCount` spends it.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::RoomEntered { room, .. } if super::combat_room(room) => state.used = false,
        EventHook::RoomEntered { .. } | EventHook::Obtained { .. } => {}
        EventHook::CombatEnd { .. } => state.used = false,
        EventHook::PlayCountModified { .. } => state.used = true,
        hook => unimplemented!("what ThrowingAxe does for {hook:?}"),
    }
}

/// `ModifyCardPlayCount`: one more play of its owner's card while unspent.
/// A fight here has one player.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::CardPlayCount { .. } if !state.used => Modify::Added(1),
        ModifyHook::CardPlayCount { .. } => Modify::Nothing,
        hook => unimplemented!("what ThrowingAxe does for {hook:?}"),
    }
}
