//! `FakeLeesWaffle`: a tenth of maximum health back, once.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `CreatureCmd.Heal` for its share of its owner's maximum
/// health.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::HealPercent(V::Relic::FAKE_LEES_WAFFLE_HEAL));
    }
}
