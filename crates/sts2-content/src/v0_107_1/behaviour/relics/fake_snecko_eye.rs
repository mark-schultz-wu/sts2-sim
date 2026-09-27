//! `FakeSneckoEye`: Confused as each fight opens.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `PowerCmd.Apply<ConfusedPower>(…, 1m, …)`.
const CONFUSED: PowerAmount<super::Number> = PowerAmount::Counted(1);

/// `BeforeCombatStart`: Confused on its owner. `AfterObtained` does the same
/// inside a fight, and a run picks it up only outside one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CombatStarting { .. } = hook {
        asked.push(HookEffect::ApplyToOwner(Power::ConfusedPower, CONFUSED));
    }
}
