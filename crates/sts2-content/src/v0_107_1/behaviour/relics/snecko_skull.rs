//! `SneckoSkull`: one more Poison with every Poison its owner applies.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<PoisonPower>(1)`.
const POISON: Number = 1;

/// `ModifyPowerAmountGivenAdditive`: one more of Poison its owner gives.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountGivenAdditive {
            power: Power::PoisonPower,
            giver,
            player,
            ..
        } if giver == player => Modify::Added(POISON),
        _ => Modify::Nothing,
    }
}

/// `AfterModifyingPowerAmountGiven` only flashes.
pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained | EventHook::PowerAmountGivenModified { .. } => {}
        hook => unimplemented!("what SneckoSkull does for {hook:?}"),
    }
}
