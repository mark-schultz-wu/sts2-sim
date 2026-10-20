//! `HandDrill`: Vulnerable on an enemy whose block its owner breaks.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::ids::PowerIds as _;

/// `AfterDamageGiven`: a hit from its owner, or its owner's pet, that breaks
/// an enemy's block gives that enemy Vulnerable.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::DamageGiven {
            dealer,
            pet_of,
            target,
            player,
            block_broken: true,
            ..
        } if (dealer == Some(player) || pet_of == Some(player)) && target != player => {
            asked.push(HookEffect::ApplyPower {
                target,
                power: V::Power::VULNERABLE_POWER,
                amount: PowerAmount::Counted(V::Relic::HAND_DRILL_VULNERABLE_POWER),
                applier: Some(player),
            });
        }
        EventHook::DamageGiven { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what HandDrill does for {hook:?}"),
    }
}
