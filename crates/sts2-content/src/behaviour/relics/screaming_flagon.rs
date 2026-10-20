//! `ScreamingFlagon`: damage to every enemy at the end of a turn that
//! leaves the hand empty.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, SideKinds as _};

/// `BeforeSideTurnEnd` of its owner's side with an empty hand:
/// `CreatureCmd.Damage` over `HittableEnemies`, from its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::BeforeSideTurnEnd { side, hand: 0, .. } if side == player => {
            asked.push(HookEffect::DamageOpponents(
                V::Relic::SCREAMING_FLAGON_DAMAGE,
            ));
        }
        EventHook::BeforeSideTurnEnd { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ScreamingFlagon does for {hook:?}"),
    }
}
