//! `Sozu`: one more energy each turn, and no potion.

use sts2_core::{DecisionHook, HookResponse, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `ShouldProcurePotion`: no for its owner. A relic's owner is the player,
/// and a run here has one.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldProcurePotion { .. } => HookResponse::Decision(false),
        hook => unimplemented!("what Sozu decides for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what Sozu does for {hook:?}"),
    }
}
