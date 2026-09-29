//! `BowlerHat`: a quarter more of every gold gain.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DynamicVar("GoldIncrease", 1.25)`.
const GOLD_INCREASE: f64 = 1.25;

/// `ModifyGoldGained`, which scales every gain its owner makes.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::GoldGained => Modify::Scaled(GOLD_INCREASE),
        hook => unimplemented!("what BowlerHat does for {hook:?}"),
    }
}

/// Its `AfterModifyingGoldGained` only flashes, and `AfterObtained` reaches
/// every relic and does nothing here.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::GoldGainModified | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BowlerHat does for {hook:?}"),
    }
}
