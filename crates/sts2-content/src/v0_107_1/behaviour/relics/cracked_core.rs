//! `CrackedCore`: the Defect's Lightning as each fight opens.

use sts2_core::{EventHook, HookEffect, SideKinds};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::orbs::Orb;
use super::super::super::generated::runstate::CombatSide;

/// `DynamicVar("Lightning", 1)`: how many it channels.
const LIGHTNING: usize = 1;

/// `BeforeSideTurnStart`, which channels Lightning before the owner's first
/// turn and no other. A relic's owner is the player, and a fight here has
/// one, so the player's side starting is always its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::BeforeSideTurnStart { side, turn }
            if side == <CombatSide as SideKinds>::PLAYER && turn <= 1 =>
        {
            asked.extend((0..LIGHTNING).map(|_| HookEffect::Channel(Orb::LightningOrb)));
        }
        _ => {}
    }
}
