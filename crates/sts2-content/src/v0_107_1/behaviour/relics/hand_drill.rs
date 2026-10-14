//! `HandDrill`: Vulnerable on an enemy whose block its owner breaks.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `PowerVar<VulnerablePower>(2)`.
const VULNERABLE: Number = 2;

/// `AfterDamageGiven`: a hit from its owner, or its owner's pet, that breaks
/// an enemy's block gives that enemy Vulnerable.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
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
                power: Power::VulnerablePower,
                amount: PowerAmount::Counted(VULNERABLE),
                applier: Some(player),
            });
        }
        EventHook::DamageGiven { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what HandDrill does for {hook:?}"),
    }
}
