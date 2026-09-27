//! `FakeBloodVial`: a point of health back on the first turn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `HealVar(1)`.
const HEAL: Number = 1;

/// `AfterPlayerTurnStartLate`: its heal on its owner's first turn. A fight
/// here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStartLate { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::Heal(HEAL));
    }
}
