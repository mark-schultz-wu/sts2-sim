//! `Planisphere`: health back on entering a room through a `?` point.

use sts2_core::{EventHook, HookEffect, MapPointKinds as _};

use crate::behaviour::relics::Number;
use crate::generated::runstate::MapPointType;

/// `HealVar(5)`.
const HEAL: Number = 5;

/// `AfterRoomEntered`: `CreatureCmd.Heal` of five, where the map point the
/// room stands on is an unknown one, whatever room it rolled.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
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
