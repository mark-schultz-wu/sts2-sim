//! `Brimstone`: Strength for its owner each turn, and some for every enemy.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount, SideKinds as _};

use crate::ids::PowerIds as _;

/// `AfterSideTurnStart` of its owner's side: `PowerCmd.Apply<StrengthPower>`
/// on the owner from the owner, then on every living opponent from nobody.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::SideTurnStart { side, .. } if side == player => {
            asked.push(HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::BRIMSTONE_SELF_STRENGTH),
            ));
            asked.push(HookEffect::ApplyToLivingOpponents(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::BRIMSTONE_ENEMY_STRENGTH),
            ));
        }
        EventHook::SideTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Brimstone does for {hook:?}"),
    }
}
