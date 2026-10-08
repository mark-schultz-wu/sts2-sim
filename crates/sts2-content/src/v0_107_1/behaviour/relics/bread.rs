//! `Bread`: two energy fewer on the first turn of a fight, and one more on
//! every turn after.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar("GainEnergy", 1)`.
const GAIN: Number = 1;

/// `EnergyVar("LoseEnergy", 2)`.
const LOSE: Number = 2;

/// `AfterSideTurnStart`: its energy lost on its owner's first turn. A
/// relic's owner is the player, and a fight here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn: 1,
        ..
    } = hook
    {
        asked.push(HookEffect::LoseEnergy(LOSE));
    }
}

/// `ModifyMaxEnergy`: its energy, save on its owner's first turn.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { turn: 1, .. } => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(GAIN),
        hook => unimplemented!("what Bread does for {hook:?}"),
    }
}
