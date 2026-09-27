//! `VeryHotCocoa`: energy on the first turn of each fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(4)`.
const ENERGY: Number = 4;

/// `AfterSideTurnStart`: `PlayerCmd.GainEnergy` while its owner's
/// `TurnNumber` is at most one. A relic's owner is the player, and a fight
/// here has one, so the player's side holds its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } if turn <= 1 => asked.push(HookEffect::GainEnergy(ENERGY)),
        _ => {}
    }
}
