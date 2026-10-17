//! `ForgottenSoul`: a hit on a random enemy for each card exhausted.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardExhausted`: `CreatureCmd.Damage` from its owner on an enemy
/// that can be hit, drawn off `CombatTargets`. A fight here has one player,
/// so every card exhausted is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardExhausted { .. } => asked.push(HookEffect::DamageRandomOpponent {
            amount: V::Relic::FORGOTTEN_SOUL_DAMAGE,
            unblockable: false,
            from_owner: true,
        }),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ForgottenSoul does for {hook:?}"),
    }
}
