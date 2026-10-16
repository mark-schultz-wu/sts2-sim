//! `CrackedCore`: the Defect's Lightning as each fight opens.

use sts2_core::{EventHook, HookEffect, SideKinds};

use crate::generated::runstate::CombatSide;
use crate::ids::OrbIds as _;

/// `DynamicVar("Lightning", 1)`: how many it channels.
const LIGHTNING: usize = 1;

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
            asked.extend((0..LIGHTNING).map(|_| HookEffect::Channel(V::Orb::LIGHTNING_ORB)));
        }
        _ => {}
    }
}
