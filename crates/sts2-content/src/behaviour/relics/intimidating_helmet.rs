//! `IntimidatingHelmet`: block before each play that spends two energy or
//! more.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeCardPlayed`: `CreatureCmd.GainBlock` on its owner when the play's
/// `Resources.EnergyValue` reaches its energy. A relic's owner is the player,
/// and a fight here has one, so every card played is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlaying { spent, .. } if spent >= V::Relic::INTIMIDATING_HELMET_ENERGY => {
            asked.push(HookEffect::GainBlock(V::Relic::INTIMIDATING_HELMET_BLOCK));
        }
        EventHook::CardPlaying { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what IntimidatingHelmet does for {hook:?}"),
    }
}
