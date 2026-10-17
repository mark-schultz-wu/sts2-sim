//! `PhilosophersStone`: one more energy each turn, and Strength for every
//! enemy.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `AfterRoomEntered` in a combat room: `PowerCmd.Apply<StrengthPower>` on
/// every living opponent from nobody. `AfterCreatureAddedToCombat` gives an
/// enemy that joins the same.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let strength = PowerAmount::Counted(V::Relic::PHILOSOPHERS_STONE_STRENGTH_POWER);
    match hook {
        EventHook::RoomEntered { room, .. } if crate::behaviour::relics::combat_room(room) => {
            asked.push(HookEffect::ApplyToLivingOpponents(
                V::Power::STRENGTH_POWER,
                strength,
            ));
        }
        EventHook::CreatureAdded {
            creature,
            side: CombatSide::Enemy,
            ..
        } => asked.push(HookEffect::ApplyPower {
            target: creature,
            power: V::Power::STRENGTH_POWER,
            amount: strength,
            applier: None,
        }),
        _ => {}
    }
}

/// `ModifyMaxEnergy`: its energy, on every turn.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::PHILOSOPHERS_STONE_ENERGY),
        hook => unimplemented!("what PhilosophersStone does for {hook:?}"),
    }
}
