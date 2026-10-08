//! `PhilosophersStone`: one more energy each turn, and Strength for every
//! enemy.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `PowerVar<StrengthPower>(1)`.
const STRENGTH: Number = 1;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AfterRoomEntered` in a combat room: `PowerCmd.Apply<StrengthPower>` on
/// every living opponent from nobody. `AfterCreatureAddedToCombat` gives an
/// enemy that joins the same.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    let strength = PowerAmount::Counted(STRENGTH);
    match hook {
        EventHook::RoomEntered { room, .. } if super::combat_room(room) => {
            asked.push(HookEffect::ApplyToLivingOpponents(
                Power::StrengthPower,
                strength,
            ));
        }
        EventHook::CreatureAdded {
            creature,
            side: CombatSide::Enemy,
            ..
        } => asked.push(HookEffect::ApplyPower {
            target: creature,
            power: Power::StrengthPower,
            amount: strength,
        }),
        _ => {}
    }
}

/// `ModifyMaxEnergy`: its energy, on every turn.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what PhilosophersStone does for {hook:?}"),
    }
}
