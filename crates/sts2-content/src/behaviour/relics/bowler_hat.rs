//! `BowlerHat`: a quarter more of every gold gain.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `DynamicVar("GoldIncrease", 1.25)`.
const GOLD_INCREASE: sts2_core::Fixed = sts2_core::Fixed::new(125, 2);

/// `ModifyGoldGained`, which scales every gain its owner makes.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::GoldGained => Modify::Scaled(GOLD_INCREASE),
        hook => unimplemented!("what BowlerHat does for {hook:?}"),
    }
}

/// Its `AfterModifyingGoldGained` only flashes, and `AfterObtained` reaches
/// every relic and does nothing here.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::GoldGainModified | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BowlerHat does for {hook:?}"),
    }
}
