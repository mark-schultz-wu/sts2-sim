//! `Chandelier`: energy on the third turn of each fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(3)`.
const ENERGY: Number = 3;

/// `_energyTurn`.
const TURN: <V as sts2_core::ContentVersion>::Count = 3;

/// `AfterSideTurnStart` on its owner's side: its energy on the owner's third
/// turn.
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
