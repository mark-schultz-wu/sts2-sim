//! `Brimstone`: Strength for its owner each turn, and some for every enemy.

use sts2_core::{EventHook, HookEffect, PowerAmount, SideKinds as _};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<StrengthPower>("SelfStrength", 2)`.
const SELF_STRENGTH: Number = 2;

/// `PowerVar<StrengthPower>("EnemyStrength", 1)`.
const ENEMY_STRENGTH: Number = 1;

/// `AfterSideTurnStart` of its owner's side: `PowerCmd.Apply<StrengthPower>`
/// on the owner from the owner, then on every living opponent from nobody.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::SideTurnStart { side, .. } if side == player => {
            asked.push(HookEffect::ApplyToOwner(
                Power::StrengthPower,
                PowerAmount::Counted(SELF_STRENGTH),
            ));
            asked.push(HookEffect::ApplyToLivingOpponents(
                Power::StrengthPower,
                PowerAmount::Counted(ENEMY_STRENGTH),
            ));
        }
        EventHook::SideTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Brimstone does for {hook:?}"),
    }
}
