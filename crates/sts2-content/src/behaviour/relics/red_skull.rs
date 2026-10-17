//! `RedSkull`: Strength while its owner stands at half health or below.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PlayerHealth, PowerAmount};

use crate::ids::PowerIds as _;

/// `StrengthApplied`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    strength_applied: bool,
}

/// `AfterRoomEntered` in a fight's room and `AfterCurrentHpChanged` inside
/// a fight run `ModifyStrengthIfNecessary`. `AfterCombatEnd` forgets what it
/// lent, which the fight took with it.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, player, .. }
            if crate::behaviour::relics::combat_room(room) =>
        {
            modify_strength::<V>(state, player, asked);
        }
        EventHook::CurrentHpChanged {
            in_fight: true,
            player,
            ..
        } => modify_strength::<V>(state, player, asked),
        EventHook::CombatEnd { .. } => state.strength_applied = false,
        _ => {}
    }
}

/// `ModifyStrengthIfNecessary`: its Strength on its owner at half health or
/// below, `DynamicVar("HpThreshold", 50)`, and taken back above it.
fn modify_strength<V: crate::Build>(
    state: &mut State,
    player: PlayerHealth<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let above = 2 * i64::from(player.hp) > i64::from(player.max_hp);
    match (above, state.strength_applied) {
        (true, true) => {
            asked.push(HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(-V::Relic::RED_SKULL_STRENGTH_POWER),
            ));
            state.strength_applied = false;
        }
        (false, false) => {
            asked.push(HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::RED_SKULL_STRENGTH_POWER),
            ));
            state.strength_applied = true;
        }
        _ => {}
    }
}
