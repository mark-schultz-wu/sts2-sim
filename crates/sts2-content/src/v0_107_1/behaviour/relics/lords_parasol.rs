//! `LordsParasol`: everything a merchant sells, bought for nothing as its
//! owner walks in.

use sts2_core::{EventHook, HookEffect, HookEffects};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;

/// `AfterRoomEntered` for a merchant's room: `PurchaseEverything`. A relic's
/// owner is the player, and a run here has one, so the inventory is its
/// owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut HookEffects<V>) {
    match hook {
        EventHook::RoomEntered {
            room: RoomType::Shop,
            ..
        } => asked.push(HookEffect::PurchaseEverything),
        EventHook::RoomEntered { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what LordsParasol does for {hook:?}"),
    }
}
