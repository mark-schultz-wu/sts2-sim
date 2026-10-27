//! `Sozu`: one more energy each turn, and no potion.

use crate::vars::RelicVars as _;
use sts2_core::{DecisionHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ShouldProcurePotion`: no for its owner. A relic's owner is the player,
/// and a run here has one.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldProcurePotion { .. } => HookResponse::Decision(false),
        hook => unimplemented!("what Sozu decides for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::SOZU_ENERGY),
        hook => unimplemented!("what Sozu does for {hook:?}"),
    }
}
