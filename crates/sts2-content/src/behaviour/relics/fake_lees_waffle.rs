//! `FakeLeesWaffle`: a tenth of maximum health back, once.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `HealVar(10)`, in hundredths of maximum health.
const HEAL: Number = 10;

/// `AfterObtained`: `CreatureCmd.Heal` for its share of its owner's maximum
/// health.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::HealPercent(HEAL));
    }
}
