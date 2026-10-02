//! `Ectoplasm`: an energy each turn, and no gold ever again.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `ModifyMaxEnergy`: one more. `ModifyGoldGained`: nothing, whatever the
/// gain was.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        ModifyHook::GoldGained => Modify::Replaced(0),
        hook => unimplemented!("what Ectoplasm does for {hook:?}"),
    }
}

/// Its `AfterModifyingGoldGained` only flashes, and `AfterObtained` reaches
/// every relic and does nothing here.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::GoldGainModified | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Ectoplasm does for {hook:?}"),
    }
}
