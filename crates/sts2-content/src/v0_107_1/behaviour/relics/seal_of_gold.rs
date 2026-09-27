//! `SealOfGold`: gold spent for energy as each of the player's turns starts.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `GoldVar(5)`.
const GOLD: Number = 5;

/// `AfterSideTurnStart`, on the side that holds its owner, the one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        ..
    } = hook
    {
        asked.push(HookEffect::GainEnergyForGold {
            energy: ENERGY,
            gold: GOLD,
        });
    }
}
