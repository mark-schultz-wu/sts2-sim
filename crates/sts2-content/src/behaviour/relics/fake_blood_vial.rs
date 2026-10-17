//! `FakeBloodVial`: a point of health back on the first turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStartLate`: its heal on its owner's first turn. A fight
/// here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStartLate { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::Heal(V::Relic::FAKE_BLOOD_VIAL_HEAL));
    }
}
