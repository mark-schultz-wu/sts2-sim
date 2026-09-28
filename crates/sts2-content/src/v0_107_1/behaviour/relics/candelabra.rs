//! `Candelabra`: energy as its owner's second turn starts.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(2)`.
const ENERGY: Number = 2;

/// `_energyTurn`.
const TURN: u32 = 2;

/// `AfterSideTurnStart` on its owner's second turn: `PlayerCmd.GainEnergy`.
/// A fight here has one player, who owns it.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn == TURN
    {
        asked.push(HookEffect::GainEnergy(ENERGY));
    }
}
