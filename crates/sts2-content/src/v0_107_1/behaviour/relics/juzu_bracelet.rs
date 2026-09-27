//! `JuzuBracelet`: no fight behind an unknown map point.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;

/// `ModifyUnknownMapPointRoomTypes`, which takes the monster room out.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::UnknownRoomTypes = hook {
        asked.push(HookEffect::RemoveRoomType(RoomType::Monster));
    }
}
