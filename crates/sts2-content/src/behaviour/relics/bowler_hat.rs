//! `BowlerHat`: a quarter more of every gold gain.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyGoldGained`, which scales every gain its owner makes.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::GoldGained { player } if player == owner => {
            Modify::Scaled(V::Relic::BOWLER_HAT_GOLD_INCREASE)
        }
        ModifyHook::GoldGained { .. } => Modify::Nothing,
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
