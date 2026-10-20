//! `CrackedCore`: the Defect's Lightning as each fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, SideKinds};

use crate::generated::runstate::CombatSide;
use crate::ids::OrbIds as _;

/// `BeforeSideTurnStart`, which channels Lightning before the owner's first
/// turn and no other. A relic's owner is the player, and a fight here has
/// one, so the player's side starting is always its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeSideTurnStart { side, turn, .. }
            if side == <CombatSide as SideKinds>::PLAYER && turn <= 1 =>
        {
            asked.extend(
                (0..(V::Relic::CRACKED_CORE_LIGHTNING as usize))
                    .map(|_| HookEffect::Channel(V::Orb::LIGHTNING_ORB)),
            );
        }
        _ => {}
    }
}
