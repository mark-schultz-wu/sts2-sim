//! `ForgottenSoul`: a hit on a random enemy for each card exhausted.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DamageVar(1, ValueProp.Unpowered)`.
const DAMAGE: Number = 1;

/// `AfterCardExhausted`: `CreatureCmd.Damage` from its owner on an enemy
/// that can be hit, drawn off `CombatTargets`. A fight here has one player,
/// so every card exhausted is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardExhausted { .. } => asked.push(HookEffect::DamageRandomOpponent {
            amount: DAMAGE,
            unblockable: false,
            from_owner: true,
        }),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ForgottenSoul does for {hook:?}"),
    }
}
