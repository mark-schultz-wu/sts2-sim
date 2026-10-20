//! `SneckoSkull`: one more Poison with every Poison its owner applies.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::ids::PowerIds as _;

/// `PowerVar<PoisonPower>(1)`.
const POISON: Number = 1;

/// `ModifyPowerAmountGivenAdditive`: one more of Poison its owner gives.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountGivenAdditive {
            power: __c0,
            giver,
            player,
            ..
        } if __c0 == V::Power::POISON_POWER && (giver == player) => Modify::Added(POISON),
        _ => Modify::Nothing,
    }
}

/// `AfterModifyingPowerAmountGiven` only flashes.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } | EventHook::PowerAmountGivenModified { .. } => {}
        hook => unimplemented!("what SneckoSkull does for {hook:?}"),
    }
}
