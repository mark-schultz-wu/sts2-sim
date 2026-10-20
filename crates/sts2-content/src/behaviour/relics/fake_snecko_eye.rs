//! `FakeSneckoEye`: Confused as each fight opens.

use crate::ids::PowerIds as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `PowerCmd.Apply<ConfusedPower>(…, 1m, …)`.
const CONFUSED: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(1);

/// `BeforeCombatStart`: Confused on its owner. `AfterObtained` does the same
/// inside a fight, and a run picks it up only outside one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatStarting { .. } = hook {
        asked.push(HookEffect::ApplyToOwner(V::Power::CONFUSED_POWER, CONFUSED));
    }
}
