//! `Planisphere`: health back on entering a room through a `?` point.

use sts2_core::{EventHook, HookEffect, MapPointKinds as _};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::MapPointType;
use super::Number;

/// `HealVar(5)`.
const HEAL: Number = 5;

/// `AfterRoomEntered`: `CreatureCmd.Heal` of five, where the map point the
/// room stands on is an unknown one, whatever room it rolled.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::RoomEntered { point, .. } => {
            if point == Some(MapPointType::UNKNOWN) {
                asked.push(HookEffect::Heal(HEAL));
            }
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Planisphere does for {hook:?}"),
    }
}
