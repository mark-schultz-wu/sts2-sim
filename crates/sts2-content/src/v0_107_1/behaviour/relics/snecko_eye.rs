//! `SneckoEye`: Confused as each fight opens, and two more cards in each
//! hand.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerCmd.Apply<ConfusedPower>(…, 1m, …)`.
const CONFUSED: PowerAmount<Number> = PowerAmount::Counted(1);

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `BeforeCombatStart`: Confused on its owner. `AfterObtained` does the same
/// inside a fight, and a run picks it up only outside one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CombatStarting { .. } => {
            asked.push(HookEffect::ApplyToOwner(Power::ConfusedPower, CONFUSED));
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what SneckoEye does for {hook:?}"),
    }
}

/// `ModifyHandDraw`: two more cards on every turn.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { .. } => Modify::Added(CARDS),
        hook => unimplemented!("what SneckoEye does for {hook:?}"),
    }
}
