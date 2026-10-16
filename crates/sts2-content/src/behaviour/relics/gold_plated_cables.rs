//! `GoldPlatedCables`: the first orb's passive triggers once more.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyOrbPassiveTriggerCounts`: one more for `Orbs[0]`. A fight here has
/// one player, so every orb is its owner's.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::OrbPassiveTriggerCount { first: true } => Modify::Added(1),
        ModifyHook::OrbPassiveTriggerCount { first: false } => Modify::Nothing,
        hook => unimplemented!("what GoldPlatedCables does for {hook:?}"),
    }
}

/// `AfterModifyingOrbPassiveTriggerCount` only flashes it, and the fight
/// raises it nowhere. `AfterObtained` reaches every relic, and its own does
/// nothing.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what GoldPlatedCables does for {hook:?}"),
    }
}
