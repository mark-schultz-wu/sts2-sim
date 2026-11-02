//! `Ectoplasm`: an energy each turn, and no gold ever again.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyMaxEnergy`: one more. `ModifyGoldGained`: nothing, whatever the
/// gain was.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::ECTOPLASM_ENERGY),
        ModifyHook::GoldGained { player } if player != owner => Modify::Nothing,
        ModifyHook::GoldGained { .. } => Modify::Replaced(0),
        hook => unimplemented!("what Ectoplasm does for {hook:?}"),
    }
}

/// Its `AfterModifyingGoldGained` only flashes, and `AfterObtained` reaches
/// every relic and does nothing here.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::GoldGainModified | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Ectoplasm does for {hook:?}"),
    }
}
