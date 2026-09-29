//! `ChemicalX`: two more on every X its owner's cards resolve.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DynamicVar("Increase", 2)`.
const INCREASE: Number = 2;

/// `ModifyXValue`: its increase on every X. A fight here has one player, so
/// every card is its owner's.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::XValue => Modify::Added(INCREASE),
        hook => unimplemented!("what ChemicalX does for {hook:?}"),
    }
}

/// `BeforeCardPlayed` only flashes it, and its own `AfterObtained` does
/// nothing.
pub(super) fn event_hook(hook: EventHook<V>, _asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } | EventHook::CardPlaying { .. } => {}
        hook => unimplemented!("what ChemicalX does for {hook:?}"),
    }
}
