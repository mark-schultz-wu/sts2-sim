//! `PhylacteryUnbound`: Bound Phylactery refined, a larger Osty as each fight
//! opens and more each turn.

use sts2_core::{EventHook, HookEffect, SideKinds};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `SummonVar("StartOfCombat", 5)`.
const START_OF_COMBAT: Number = 5;

/// `SummonVar("StartOfTurn", 2)`.
const START_OF_TURN: Number = 2;

/// `BeforeCombatStart`, and `AfterSideTurnStart` on every turn of its
/// owner's. Both are `OstyCmd.Summon` for its owner, the only player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CombatStarting => asked.push(HookEffect::Summon(START_OF_COMBAT)),
        EventHook::SideTurnStart { side, .. } if side == <CombatSide as SideKinds>::PLAYER => {
            asked.push(HookEffect::Summon(START_OF_TURN));
        }
        _ => {}
    }
}
