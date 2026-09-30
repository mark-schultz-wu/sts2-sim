//! `Pantograph`: a heal as a boss fight opens.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `HealVar(25)`.
const HEAL: Number = 25;

/// `BeforeCombatStart` in a boss's room: its heal. Its `AfterRoomEntered`
/// sets only how the relic is drawn.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CombatStarting {
        room: RoomType::Boss,
        ..
    } = hook
    {
        asked.push(HookEffect::Heal(HEAL));
    }
}
