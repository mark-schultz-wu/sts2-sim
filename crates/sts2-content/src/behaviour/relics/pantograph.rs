//! `Pantograph`: a heal as a boss fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

/// `BeforeCombatStart` in a boss's room: its heal. Its `AfterRoomEntered`
/// sets only how the relic is drawn.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatStarting {
        room: RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::Heal(V::Relic::PANTOGRAPH_HEAL));
    }
}
