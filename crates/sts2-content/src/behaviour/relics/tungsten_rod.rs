//! `TungstenRod`: one less HP lost from every loss its owner takes.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `DynamicVar("HpLossReduction", 1)`.
const REDUCTION: Number = 1;

/// `ModifyHpLostAfterOsty`: its reduction off a loss its owner takes,
/// `Math.Max(0, amount - 1)`.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HpLostAfterOsty { target, player, .. } if target == player => {
            Modify::Less(REDUCTION)
        }
        ModifyHook::HpLostAfterOsty { .. } => Modify::Nothing,
        hook => unimplemented!("what TungstenRod does for {hook:?}"),
    }
}

/// Its `AfterModifyingHpLostAfterOsty` only flashes, and the fight asks it
/// nothing else.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TungstenRod does for {hook:?}"),
    }
}
