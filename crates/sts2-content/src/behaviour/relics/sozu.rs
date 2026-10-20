//! `Sozu`: one more energy each turn, and no potion.

use sts2_core::{DecisionHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `ShouldProcurePotion`: no for its owner. A relic's owner is the player,
/// and a run here has one.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldProcurePotion { .. } => HookResponse::Decision(false),
        hook => unimplemented!("what Sozu decides for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what Sozu does for {hook:?}"),
    }
}
