//! `Candelabra`: energy as its owner's second turn starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `_energyTurn`.
const TURN: u32 = 2;

/// `AfterSideTurnStart` on its owner's second turn: `PlayerCmd.GainEnergy`.
/// A fight here has one player, who owns it.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn == TURN
    {
        asked.push(HookEffect::GainEnergy(V::Relic::CANDELABRA_ENERGY));
    }
}
