//! `ChemicalX`: two more on every X its owner's cards resolve.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyXValue`: its increase on every X of its owner's cards.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::XValue { player } if player == owner => {
            Modify::Added(V::Relic::CHEMICAL_X_INCREASE)
        }
        ModifyHook::XValue { .. } => Modify::Nothing,
        hook => unimplemented!("what ChemicalX does for {hook:?}"),
    }
}

/// `BeforeCardPlayed` only flashes it, and its own `AfterObtained` does
/// nothing.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } | EventHook::CardPlaying { .. } => {}
        hook => unimplemented!("what ChemicalX does for {hook:?}"),
    }
}
