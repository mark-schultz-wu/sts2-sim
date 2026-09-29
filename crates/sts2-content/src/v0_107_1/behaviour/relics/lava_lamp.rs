//! `LavaLamp`: a fight's card reward upgraded, where no hit got through.

use sts2_core::{CardTable as _, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;

/// What it saves, and the room it stands in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct State {
    /// `TookDamageThisCombat`.
    took_damage: bool,
    /// `RunState.CurrentRoom is CombatRoom`, as the last room entered says.
    combat_room: bool,
}

/// `AfterRoomEntered` starts the fight's count again. `AfterDamageReceived`
/// notes health its owner lost to a hit that was not unblockable, in a
/// combat room. `TryModifyCardRewardOptionsLate` upgrades each reward card
/// that can be, in a combat room where nothing got through.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, .. } => {
            state.took_damage = false;
            state.combat_room =
                matches!(room, RoomType::Monster | RoomType::Elite | RoomType::Boss);
        }
        EventHook::DamageReceived {
            target,
            unblocked,
            unblockable,
            player,
            ..
        } => {
            if state.combat_room && target == player && unblocked > 0 && !unblockable {
                state.took_damage = true;
            }
        }
        EventHook::CardRewardOption { card, .. } => {
            if state.combat_room && !state.took_damage && card.upgrade < card.card.max_upgrade() {
                asked.push(HookEffect::UpgradeCard);
            }
        }
        EventHook::Obtained { .. }
        | EventHook::CardRewardOptionsModified
        | EventHook::CardRewardPick { .. } => {}
        hook => unimplemented!("what LavaLamp does for {hook:?}"),
    }
}
